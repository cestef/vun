//! Repository automation, run through the `cargo xtask` alias.

mod corpus;
mod explanations;
mod lines;
mod report;
mod size;
#[cfg(test)]
mod tests;

use std::io;
use std::process::ExitCode;

use vun_command::{Args, Command, Outcome, Program, Shell};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_term::{Bytes, Style, Text, Verb};

use corpus::Corpus;
use lines::Oversized;
use report::{PASSED, Tally};
use size::Measured;

const OVERSIZED: DiagId = DiagId::new("xtask.lines.oversized");
const OVER_BUDGET: DiagId = DiagId::new("xtask.size.over_budget");

const PROGRAM: Program<DiagCtxt> = Program {
    configure: vun_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    root: Command::group(
        "cargo xtask",
        "Repository automation.",
        &[
            Command::run(
                "corpus",
                "run corpus/accept and corpus/reject through the stages that exist",
                run_corpus,
            ),
            Command::run("lines", "fail on source files too long to stay one concern", check_lines),
            Command::run("size", "fail when the distributed vun outgrows its budget", check_size),
        ],
    ),
};

fn run_corpus(_: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let outcomes = Corpus::at(&repo_root()).outcomes()?;
    let mut tally = Tally::default();
    for o in &outcomes {
        tally.count(&o.verdict);
        for line in o.verdict.lines(Text::of(Style::Strong, o.path.display().to_string())) {
            shell.out.line(&line)?;
        }
    }
    shell.out.line(&tally.text())?;
    let coverage = Corpus::at(&repo_root()).coverage()?;
    shell
        .out
        .line(&Verb::under(Text::of(Style::Strong, vun_harness::ACCEPT).plain(format!(": {coverage}"))))?;
    Ok(if tally.failed == 0 { Outcome::Success } else { Outcome::Failure })
}

fn check_lines(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let oversized = Oversized::all(&repo_root())?;
    for o in &oversized {
        let msg = Msg::new()
            .code(o.path.display().to_string())
            .text(" has ")
            .num(o.lines)
            .text(" lines, more than ")
            .num(lines::MAX_LINES);
        let help = Msg::new().text("split it into a directory module, one concern per file");
        diag.error(Diagnostic::error(OVERSIZED, msg).child(ChildKind::Help, help));
    }
    if oversized.is_empty() {
        shell.status(PASSED, Text::from(format!("every source file has at most {} lines", lines::MAX_LINES)));
    }
    Ok(if oversized.is_empty() { Outcome::Success } else { Outcome::Failure })
}

fn check_size(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let measured = Measured::build(&repo_root())?;
    let within = measured.bytes <= size::BUDGET;
    if within {
        let what = Text::of(Style::Strong, measured.path.display().to_string());
        let under = format!(
            " is {}, {} under the budget of {}",
            Bytes(measured.bytes),
            Bytes(size::BUDGET - measured.bytes),
            Bytes(size::BUDGET)
        );
        shell.status(PASSED, what.plain(under));
    } else {
        let msg = Msg::new()
            .code(measured.path.display().to_string())
            .text(" is ")
            .num(measured.bytes)
            .text(" bytes, over the budget of ")
            .num(size::BUDGET);
        let help = Msg::new().text("find what grew; raising the budget takes a decision (decision 131)");
        diag.error(Diagnostic::error(OVER_BUDGET, msg).child(ChildKind::Help, help));
    }
    Ok(if within { Outcome::Success } else { Outcome::Failure })
}

fn main() -> ExitCode {
    PROGRAM.main(&mut DiagCtxt::new())
}
