//! `vun debug <stage>`: each representation of a file as text, with origins.

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use vun_command::{Args, Outcome};
use vun_hir::db::Package;
use vun_mir::{Body, Concrete, Mir, Unbuilt};
use vun_origin::OriginId;
use vun_sema::layout::Shapes;
use vun_sema::member::Members;
use vun_span::SourceMap;
use vun_term::Shell;

use super::super::FILE;
use crate::Session;

/// What debug commands report.
pub(super) const DUMP: &str = "dump";

/// What `vun debug <stage>` prints.
#[derive(Clone, Copy)]
pub(super) enum Stage {
    Tokens,
    Cst,
    Hir,
    Mir,
    Program,
    Lir,
    Code,
    Frames,
    Layout,
    Manifest,
    Plan,
}

impl Stage {
    /// Its command's name.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Cst => "cst",
            Self::Hir => "hir",
            Self::Mir => "mir",
            Self::Program => "program",
            Self::Lir => "lir",
            Self::Code => "code",
            Self::Frames => "frames",
            Self::Layout => "layout",
            Self::Manifest => "manifest",
            Self::Plan => "plan",
        }
    }

    /// Reports this stage of the file `args` names as a dump.
    pub(super) fn show(
        self,
        session: &mut Session,
        args: &Args,
        shell: &mut Shell<'_>,
    ) -> io::Result<Outcome> {
        shell.document(DUMP, self.name().to_owned(), |out| self.dump(session, args, out))
    }

    /// Prints this stage of the file `args` names.
    pub(super) fn dump(self, session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
        let path = args.value(&FILE).expect("positional parameters are present when a command runs");
        let Some(file) = session.load(Path::new(path)) else { return Ok(Outcome::Failure) };
        if let Self::Manifest = self {
            if let Some(manifest) = session.manifest(file) {
                writeln!(out, "{manifest:#?}")?;
            }
            return Ok(session.outcome());
        }
        if let Self::Plan = self {
            for action in session.plan(file).map(|p| p.actions).unwrap_or_default() {
                // What no literal spells, such as a type, shows as diagnostics show values.
                let span = vun_span::Span::new(file, 0, 0);
                let package = |p| session.db.get::<Package>(&p).name;
                match action.literal(span, &package).and_then(|l| l.source().ok()) {
                    Some(source) => writeln!(out, "{source}")?,
                    None => writeln!(out, "{action}")?,
                }
            }
            return Ok(session.outcome());
        }
        if let Self::Layout = self {
            for (name, ty) in session.types(file) {
                let shape = session.db.get::<Shapes>(&ty);
                let members = session.db.get::<Members>(&ty);
                match shape {
                    Some(shape) => writeln!(out, "{name}: {}", shape.text(members.as_deref()))?,
                    None => writeln!(out, "{name}: no layout")?,
                }
            }
            return Ok(session.outcome());
        }
        let tokens = session.lex(file);
        match self {
            Self::Tokens => write!(out, "{}", tokens.dump(session.text(file)))?,
            Self::Cst => {
                let cst = session.parse(&tokens);
                write!(out, "{}", cst.dump(&tokens, session.text(file)))?;
            }
            Self::Hir => {
                let parsed = session.parsed(file);
                session.check_names(file);
                let text = parsed.hir.dump(&parsed.resolution, session.origins, session.sources);
                write!(out, "{text}")?;
            }
            Self::Mir => {
                let bodies = session.bodies(file);
                for (f, body) in bodies {
                    session.dump_body(f.expr.origin, body, out)?;
                }
            }
            Self::Lir => {
                let lowered = session.lir(file);
                let locate = |o| session.sources.locate(session.origins.source_span(o)).at;
                match &*lowered {
                    Ok(unit) => {
                        write!(out, "{}", unit.dump(&locate))?;
                        if let Err(malformed) = unit.verify(session.origins) {
                            writeln!(out, "malformed: {malformed}")?;
                        }
                    }
                    Err(unlowered) => {
                        let at = session.sources.locate(session.origins.source_span(unlowered.at));
                        writeln!(out, "not lowered at {at}: {unlowered}")?;
                    }
                }
            }
            Self::Code => session.dump_code(file, out)?,
            Self::Frames => session.dump_frames(file, out)?,
            Self::Program => {
                let program = session.program(file);
                for &(at, symbol) in &program.exports {
                    writeln!(out, "export {symbol}: fn {at}")?;
                }
                for (at, i) in program.functions.iter().enumerate() {
                    let origin = i.f.literal.expr.origin;
                    writeln!(
                        out,
                        "fn {at} at {}",
                        session.sources.locate(session.origins.source_span(origin))
                    )?;
                    match session.db.get::<Concrete>(i) {
                        Ok(body) => session.dump_body(origin, Ok(body), out)?,
                        Err(unready) => {
                            let at = session.sources.locate(session.origins.source_span(unready.at()));
                            writeln!(out, "not ready at {at}: {unready}")?;
                            session.dump_body(origin, session.db.get::<Mir>(i), out)?;
                        }
                    }
                }
            }
            Self::Manifest | Self::Plan | Self::Layout => unreachable!("printed above"),
        }
        Ok(session.outcome())
    }
}

impl Session {
    /// Prints a function's body with the origin of each line, or where lowering it stopped.
    fn dump_body(
        &self,
        origin: OriginId,
        body: Result<Arc<Body>, Unbuilt>,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        let locate = |o| self.sources.locate(self.origins.source_span(o)).at;
        match body {
            Ok(body) => writeln!(out, "{}", body.dump(&locate)),
            Err(unbuilt) => {
                let (at, form) = (locate(origin), locate(unbuilt.at));
                let (line, col) = (form.line, form.col);
                writeln!(
                    out,
                    "fn @{}:{}: not lowered yet, at @{line}:{col}, by {}",
                    at.line, at.col, unbuilt.by
                )
            }
        }
    }
}
