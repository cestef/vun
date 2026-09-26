//! `cargo gen`: regenerates the Rust sources whose truth is in Vun sources. Built apart from the
//! compiler, from only what reading `.vun` data needs, so that a generated change breaking a
//! consumer never keeps it from running (decision 132).

mod errors;
mod generate;
mod grammar;
mod items;
mod names;
mod primitives;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::process::ExitCode;

use vun_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};
use vun_diag::{ChildKind, DiagCtxt, DiagId, DiagSink, Diagnostic, Msg};
use vun_fs::Edit;
use vun_harness::repo_root;
use vun_origin::OriginTable;
use vun_render_short::ShortRenderer;
use vun_span::SourceFiles;

/// How the generator is run, which messages suggest.
pub(crate) const RUN: &str = "cargo gen";

const STALE: DiagId = DiagId::new("generate.stale");

const CHECK: Param = Param {
    name: "check",
    short: None,
    about: "fail if a generated file is out of date instead of writing it",
    kind: ParamKind::Flag,
};

const PROGRAM: Program<DiagCtxt> = Program {
    configure: vun_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    root: Command {
        name: RUN,
        about: "regenerate Rust sources from their Vun sources",
        params: &[CHECK],
        action: Action::Run(generate_all),
    },
};

fn generate_all(diag: &mut DiagCtxt, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let root = repo_root();
    let mut outcome = Outcome::Success;
    for generator in generate::ALL {
        let generated = match generator.generate(&root) {
            Ok(generated) => generated,
            Err(d) => {
                diag.error(*d);
                outcome = Outcome::Failure;
                continue;
            }
        };
        let changes = generator.changes(&root, &generated)?;
        if changes.is_empty() {
            writeln!(out, "{} is up to date", generator.output)?;
        } else if args.flag(&CHECK) {
            let msg = Msg::new().code(generator.output).text(" is out of date with ").code(generator.source);
            let help = Msg::new().text("run ").code(RUN);
            diag.error(Diagnostic::error(STALE, msg).child(ChildKind::Help, help));
            outcome = Outcome::Failure;
        } else {
            let dir = root.join(generator.output);
            for name in &changes.differing {
                dir.join(name).put(&generated[name])?;
            }
            for path in &changes.stale {
                std::fs::remove_file(path)?;
            }
            writeln!(out, "wrote {}", generator.output)?;
        }
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut diag = DiagCtxt::new();
    let outcome = PROGRAM.run(&argv, &mut diag, &mut io::stdout().lock()).unwrap_or_else(|refusal| {
        diag.error(*refusal.diagnostic);
        refusal.outcome
    });
    let (origins, sources) = (OriginTable::new(), SourceFiles::new());
    let mut renderer = ShortRenderer::new(io::stderr());
    for d in diag.sorted(&origins) {
        renderer.emit(d, &origins, &sources);
    }
    outcome.into()
}
