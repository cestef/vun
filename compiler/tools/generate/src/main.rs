//! `cargo gen`: regenerates the Rust sources whose truth is in Vun sources. Built apart from the
//! compiler, from only what reading `.vun` data needs, so that a generated change breaking a
//! consumer never keeps it from running (decision 132).

mod asm;
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
use vun_data::Object;
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Edit;
use vun_harness::repo_root;
use vun_term::{Record, Status, Style, Text, Verb};

/// How the generator is run, which messages suggest.
pub(crate) const RUN: &str = "cargo gen";

/// A generated directory, and whether it was rewritten.
struct Generated {
    output: &'static str,
    wrote: bool,
}

impl Record for Generated {
    fn kind(&self) -> &'static str {
        "generated"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.output)
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("output", self.output).with("wrote", self.wrote)
    }
}

impl Status for Generated {
    fn verb(&self) -> Verb {
        Verb::good(if self.wrote { "Wrote" } else { "Fresh" })
    }
}

const STALE: DiagId = DiagId::new("generate.stale");

const CHECK: Param =
    Param::flag("check").about("fail if a generated file is out of date instead of writing it");

const PROGRAM: Program<DiagCtxt> = Program {
    defaults: vun_command::undefaulted,
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
            shell.status(&Generated { output: generator.output, wrote: false });
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
            shell.status(&Generated { output: generator.output, wrote: true });
        }
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    PROGRAM.main(&mut DiagCtxt::new())
}
