//! Code generation of a program (spec 13, section 7): its LIR for the chosen target, legalized
//! for the backend chosen (`Registry::choose`: the one `--backend` names, else the target's
//! preferences for the profile, `--release` or debug), then emitted.

mod unchosen;

use std::path::PathBuf;

use vun_backend::{Artifact, Emits, Emitted, Line, Lines, Request};
use vun_command::{Args, Param, Values};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_intern::Symbol;
use vun_lir::{FuncId, Scalar, Unit};
use vun_lir_passes::legalize::Legalize;
use vun_lower::Unlowered;
use vun_origin::{OriginId, OriginTable, Resolve};
use vun_span::{FileId, SourceFiles, SourceMap};
use vun_target::{Name, Output, Profile, TargetSpec};
use vun_trace::Activity;

use crate::Session;
use crate::registry::Codegen;

/// Lowering the program to LIR, as timed.
const LOWER: Activity = Activity::stage("lower");
/// Splitting what the backend cannot compute with, as timed.
const LEGALIZE: Activity = Activity::stage("legalize");
/// The backend making the program's artifacts, as timed.
const EMIT: Activity = Activity::stage("emit");

pub const BACKEND: Param = Param::option("backend")
    .about("the backend generating code, instead of the target's preferences for the profile")
    .values(Values::Choice(|| crate::Registry::with_defaults().backends().map(|b| b.info().id.0).collect()))
    .env("VUN_BACKEND");

pub const LINKER: Param = Param::option("linker")
    .about("the linker, instead of the target's")
    .values(Values::Choice(|| crate::Registry::with_defaults().linkers().map(|l| l.info().id.0).collect()))
    .env("VUN_LINKER");

pub const LIBRARY: Param = Param::option("library")
    .about("build a library other programs link instead of an executable")
    .values(Values::Choice(|| Output::ALL.iter().filter(|o| o.library()).map(|o| o.name()).collect()));

pub const RELEASE: Param = Param::flag("release").short('r').about("build fast code rather than build fast");

/// Applies `--release`, `--backend` and `--linker` to the session.
pub fn configure(session: &mut Session, args: &Args) {
    let profile = if args.flag(&RELEASE) { Profile::Release } else { Profile::Debug };
    let library = args.value(&LIBRARY).and_then(|name| Output::ALL.into_iter().find(|o| o.name() == name));
    session.codegen = Codegen {
        profile,
        backend: args.value(&BACKEND).map(Symbol::intern),
        linker: args.value(&LINKER).map(Symbol::intern),
        output: library.unwrap_or_default(),
    };
}

/// Where origins are in source, for the line information backends write.
struct SourceLines;

impl Lines for SourceLines {
    fn line(&self, origin: OriginId) -> Option<Line> {
        let at = SourceFiles::global().locate(OriginTable::global().source_span(origin));
        Some(Line { file: PathBuf::from(at.path), line: at.at.line })
    }

    fn declaration(&self, origin: OriginId) -> Vec<Symbol> {
        OriginTable::global().declaration_path(origin)
    }
}

/// Why a program has no code.
pub enum Uncoded {
    /// It does not lower to LIR.
    Unlowered(Unlowered),
    /// What stopped it was reported.
    Reported(ErrorGuaranteed),
}

/// Whether a backend lists what it generates, for `vun debug code`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listed {
    No,
    Yes,
}

impl Session {
    /// Prints what the backend generates for the program rooted at `file`, where it is text:
    /// sources, or a listing of machine code.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn dump_code(&mut self, file: FileId, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self.code(file, Listed::Yes) {
            Ok(coded) => {
                let text = |a: &&Artifact| {
                    matches!(a.kind, Emits::Source | Emits::Header | Emits::Assembly | Emits::Listing)
                };
                for artifact in coded.artifacts.iter().filter(text) {
                    out.write_all(&artifact.bytes)?;
                }
            }
            Err(Uncoded::Unlowered(unlowered)) => {
                let at = self.sources.locate(self.source_span(unlowered.at));
                writeln!(out, "not lowered at {at}: {unlowered}")?;
            }
            Err(Uncoded::Reported(_)) => {}
        }
        Ok(())
    }

    /// The artifacts of the program rooted at `file`, from the backend `self.codegen` chooses,
    /// with what made them.
    ///
    /// # Errors
    /// Returns why there is no code: the program does not lower, or a report says why.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn code(&mut self, file: FileId, listed: Listed) -> Result<Coded, Uncoded> {
        let lowered = {
            let _timer = LOWER.start();
            self.lir(file)
        };
        let target =
            self.target.clone().expect("naming a file chooses the target").map_err(Uncoded::Reported)?;
        let unit = match &*lowered {
            Ok(unit) => unit,
            Err(unlowered) => return Err(Uncoded::Unlowered(unlowered.clone())),
        };
        self.provided(file, unit).map_err(Uncoded::Reported)?;
        let backend = match self.registry.choose(self.codegen, &target) {
            Ok(backend) => backend,
            Err(unchosen) => {
                let d = self.unchosen(&unchosen, &target);
                return Err(Uncoded::Reported(self.diag.error(d)));
            }
        };
        let info = *backend.info();
        // Integers wider than the backend's are split into halves first.
        let legalizing = LEGALIZE.start();
        let mut unit = unit.clone();
        let widest =
            Scalar::int(info.caps.widest.on(&target)).expect("a backend computes with an integer width");
        if let Err(unlegal) = (Legalize {
            widest,
            endian: target.endian,
            half: info.caps.half,
            floats: info.caps.floats.on(&target),
            convention: target.convention,
        })
        .apply(&mut unit)
        {
            let msg = Msg::new()
                .text("backend ")
                .code(info.id.0)
                .text(" cannot express ")
                .text(unlegal.to_string());
            let d = Diagnostic::error(errors::BACKEND_LACKS, msg).at(unlegal.at, None);
            return Err(Uncoded::Reported(self.diag.error(d)));
        }
        drop(legalizing);
        let request = Request {
            unit: &unit,
            target: &target,
            lines: &SourceLines,
            profile: self.codegen.profile,
            listing: listed == Listed::Yes,
        };
        let mut emitted = Emitted::default();
        let backend = self.registry.backend(info.id.symbol()).expect("chosen from the registry");
        let emitting = EMIT.start();
        backend.emit(&request, &mut emitted, &mut self.diag).map_err(Uncoded::Reported)?;
        drop(emitting);
        Ok(Coded { artifacts: emitted.artifacts, frames: emitted.frames, backend: info.id, unit, target })
    }
}

/// What a backend made of a program, and what made it.
pub struct Coded {
    pub artifacts: Vec<Artifact>,
    /// Each function's frame size in bytes, when the backend reported them.
    pub frames: Vec<(FuncId, u64)>,
    pub backend: Name,
    /// The unit the artifacts are made of: the program lowered, legalized for the backend.
    pub unit: Unit,
    pub target: TargetSpec,
}
