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

use std::io;
use std::process::ExitCode;

use vun_command::{Args, Command, Outcome, Param, Program, Shell};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Edit;
use vun_harness::repo_root;
use vun_term::{Style, Text, Verb};

/// How the generator is run, which messages suggest.
pub(crate) const RUN: &str = "cargo gen";

const FRESH: Verb = Verb::good("Fresh");
const WROTE: Verb = Verb::good("Wrote");

const STALE: DiagId = DiagId::new("generate.stale");

const CHECK: Param =
    Param::flag("check").about("fail if a generated file is out of date instead of writing it");

const PROGRAM: Program<DiagCtxt> = Program {
    configure: vun_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    root: Command::run(RUN, "regenerate Rust sources from their Vun sources", generate_all).params(&[CHECK]),
};

fn generate_all(diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
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
            shell.status(FRESH, Text::of(Style::Strong, generator.output));
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
            shell.status(WROTE, Text::of(Style::Strong, generator.output));
        }
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    PROGRAM.main(&mut DiagCtxt::new())
}
