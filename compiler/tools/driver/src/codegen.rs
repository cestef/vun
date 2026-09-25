//! Code generation of a program (spec 13, section 7): its LIR for the chosen target, through the
//! backend `--backend` names, or else the target's default.

use std::path::PathBuf;
use std::sync::Arc;

use vun_backend::{Artifact, Backend, Emitted, Line, Lines};
use vun_command::{Param, ParamKind};
use vun_diag::{ChildKind, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_lower::Unlowered;
use vun_origin::{OriginId, OriginTable};
use vun_span::{FileId, SourceFiles, SourceMap};
use vun_suggest::Lookup;

use vun_lir::{FuncId, Unit};
use vun_target::TargetSpec;

use crate::Session;

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
            Ok(coded) => {
                for artifact in coded.artifacts {
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
    /// target's default, with what made them.
    ///
    /// # Errors
    /// Returns why there is no code: the program does not lower, or a report says why.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn code(&mut self, file: FileId, backend: Option<&str>) -> Result<Coded, Uncoded> {
        let lowered = self.lir(file);
        let target =
            self.target.clone().expect("naming a file chooses the target").map_err(Uncoded::Reported)?;
        if let Err(unlowered) = &*lowered {
            return Err(Uncoded::Unlowered(unlowered.clone()));
        }
        let backends: Vec<&dyn Backend> = self.registry.backends().collect();
        let name = backend.unwrap_or(target.default_backend.as_str());
        let chosen = backends.iter().position(|b| b.info().id.0 == name);
        let Some(index) = chosen else {
            let near = backends.lookup(name, |b| b.info().id.0).err().flatten();
            let msg = Msg::new().text("unknown backend ").code(name);
            let names = || Msg::new().text("backends: ").code_list(backends.iter().map(|b| b.info().id.0));
            let help = near.map_or_else(names, Msg::did_you_mean);
            let d = Diagnostic::error(errors::CLI_INVALID_VALUE, msg).child(ChildKind::Help, help);
            return Err(Uncoded::Reported(self.diag.error(d)));
        };
        let lines = SourceLines { sources: Arc::clone(&self.sources), origins: Arc::clone(&self.origins) };
        let unit = lowered.as_ref().as_ref().expect("checked above");
        let mut emitted = Emitted::default();
        backends[index]
            .emit(unit, &target, &lines, &mut emitted, &mut self.diag)
            .map_err(Uncoded::Reported)?;
        Ok(Coded { artifacts: emitted.artifacts, frames: emitted.frames, backend: index, lowered, target })
    }
}

/// What a backend made of a program, and what made it.
pub struct Coded {
    pub artifacts: Vec<Artifact>,
    /// Each function's frame size in bytes, when the backend reported them.
    pub frames: Vec<(FuncId, u64)>,
    /// The backend, by its position in the session's registry.
    pub backend: usize,
    pub lowered: Arc<Result<Unit, Unlowered>>,
    pub target: TargetSpec,
}

impl Coded {
    /// The unit the artifacts are made of.
    ///
    /// # Panics
    /// Never: only lowered programs are coded.
    #[must_use]
    pub fn unit(&self) -> &Unit {
        self.lowered.as_ref().as_ref().expect("coded programs are lowered")
    }
}
