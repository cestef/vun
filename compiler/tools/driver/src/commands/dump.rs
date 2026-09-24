//! `vun dump`: each representation of a file as text, with origins.

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use vun_command::{Action, Args, Command, Outcome};
use vun_hir::db::Package;
use vun_mir::{Body, Concrete, Mir, Unbuilt};
use vun_origin::OriginId;
use vun_sema::layout::Shapes;
use vun_sema::member::Members;
use vun_span::SourceMap;

use super::FILE;
use crate::Session;

pub(super) const DUMPS: &[Command<Session>] = &[
    Command {
        name: "tokens",
        about: "tokens and trivia with byte ranges",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Tokens.dump(s, args, out)),
    },
    Command {
        name: "cst",
        about: "the concrete syntax tree, one node per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Cst.dump(s, args, out)),
    },
    Command {
        name: "manifest",
        about: "the package manifest read from a build.vun",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Manifest.dump(s, args, out)),
    },
    Command {
        name: "plan",
        about: "the steps a build.vun's build records, one per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Plan.dump(s, args, out)),
    },
    Command {
        name: "hir",
        about: "the high-level representation, names resolved, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Hir.dump(s, args, out)),
    },
    Command {
        name: "mir",
        about: "the mid-level representation of each function, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Mir.dump(s, args, out)),
    },
    Command {
        name: "layout",
        about: "the layout of each type the file declares: sizes, offsets, tags and niches",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Layout.dump(s, args, out)),
    },
    Command {
        name: "program",
        about: "the exports of a root file and the MIR of every function they reach",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Program.dump(s, args, out)),
    },
    Command::planned("lir", "the low-level representation"),
    Command::planned("link", "the link plan"),
];

/// What `vun dump` prints.
#[derive(Clone, Copy)]
enum Stage {
    Tokens,
    Cst,
    Hir,
    Mir,
    Program,
    Layout,
    Manifest,
    Plan,
}

impl Stage {
    /// Prints this stage of the file `args` names.
    fn dump(self, session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
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
                let text = parsed.hir.dump(&parsed.resolution, &session.origins, &*session.sources);
                write!(out, "{text}")?;
            }
            Self::Mir => {
                let bodies = session.bodies(file);
                for (f, body) in bodies {
                    session.dump_body(f.expr.origin, body, out)?;
                }
            }
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
                writeln!(out, "fn @{}:{}: not lowered yet, at @{}:{}", at.line, at.col, form.line, form.col)
            }
        }
    }
}
