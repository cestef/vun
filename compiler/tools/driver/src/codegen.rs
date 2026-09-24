//! Code generation of a program (spec 13, section 7): its LIR for the chosen target, through the
//! backend `--backend` names, or else the target's default.

use std::path::PathBuf;
use std::sync::Arc;

use vun_backend::{Artifact, Backend, Line, Lines};
use vun_command::{Param, ParamKind};
use vun_diag::{ChildKind, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_lower::Unlowered;
use vun_origin::{OriginId, OriginTable};
use vun_span::{FileId, SourceFiles, SourceMap};
use vun_suggest::Lookup;

use crate::{Registry, Session};

pub const BACKEND: Param = Param {
    name: "backend",
    short: None,
    about: "the backend generating code: the target's default unless given",
    kind: ParamKind::Option,
};

/// Where origins are in source, for the line information backends write.
struct SourceLines {
    sources: Arc<SourceFiles>,
    origins: Arc<OriginTable>,
}

impl Lines for SourceLines {
    fn line(&self, origin: OriginId) -> Option<Line> {
        let at = self.sources.locate(self.origins.source_span(origin));
        Some(Line { file: PathBuf::from(at.path), line: at.at.line })
    }
}

/// Why a program has no code.
pub enum Uncoded {
    /// It does not lower to LIR.
    Unlowered(Unlowered),
    /// What stopped it was reported.
    Reported(ErrorGuaranteed),
}

impl Session {
    /// Prints what the backend generates for the program rooted at `file`, where it is text.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn dump_code(
        &mut self,
        file: FileId,
        backend: Option<&str>,
        out: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        match self.code(file, backend) {
            Ok(artifacts) => {
                for artifact in artifacts {
                    out.write_all(&artifact.bytes)?;
                }
            }
            Err(Uncoded::Unlowered(unlowered)) => {
                let at = self.sources.locate(self.origins.source_span(unlowered.at));
                writeln!(out, "not lowered at {at}: {unlowered}")?;
            }
            Err(Uncoded::Reported(_)) => {}
        }
        Ok(())
    }

    /// The artifacts of the program rooted at `file`, from the backend named `backend`, or the
    /// target's default.
    ///
    /// # Errors
    /// Returns why there is no code: the program does not lower, or a report says why.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn code(&mut self, file: FileId, backend: Option<&str>) -> Result<Vec<Artifact>, Uncoded> {
        let lowered = self.lir(file);
        let target =
            self.target.clone().expect("naming a file chooses the target").map_err(Uncoded::Reported)?;
        let unit = match &*lowered {
            Ok(unit) => unit,
            Err(unlowered) => return Err(Uncoded::Unlowered(unlowered.clone())),
        };
        let registry = Registry::with_defaults();
        let backends: Vec<&dyn Backend> = registry.backends().collect();
        let name = backend.unwrap_or(target.default_backend.as_str());
        let backend = backends.lookup(name, |b| b.info().id.0).map_err(|near| {
            let msg = Msg::new().text("unknown backend ").code(name);
            let names = || Msg::new().text("backends: ").code_list(backends.iter().map(|b| b.info().id.0));
            let help = near.map_or_else(names, Msg::did_you_mean);
            Uncoded::Reported(
                self.diag
                    .error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg).child(ChildKind::Help, help)),
            )
        })?;
        let lines = SourceLines { sources: Arc::clone(&self.sources), origins: Arc::clone(&self.origins) };
        let mut artifacts = Vec::new();
        backend.emit(unit, &target, &lines, &mut artifacts, &mut self.diag).map_err(Uncoded::Reported)?;
        Ok(artifacts)
    }
}
