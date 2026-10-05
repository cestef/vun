//! Repository automation, run through the `cargo xtask` alias.

mod cargo;
mod corpus;
mod explanations;
mod hooks;
mod install;
mod layers;
mod lines;
mod release;
mod remote;
mod report;
mod runtimes;
mod size;
mod task;
#[cfg(test)]
mod tests;
mod toolchain;
mod tree_sitter;
mod typeset;

use std::io;
use std::process::ExitCode;

use vun_command::{Args, Command, Outcome, Program, Shell};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_term::Document;

use corpus::Corpus;
use layers::{Breach, Ranks, Refreshed};
use lines::Oversized;
use report::{Judged, Passed, Tally};
use size::Measured;

const UPWARD: DiagId = DiagId::new("xtask.layers.upward");
const UNRANKED: DiagId = DiagId::new("xtask.layers.unranked");
const NO_METADATA: DiagId = DiagId::new("xtask.layers.no_metadata");
const OVERSIZED: DiagId = DiagId::new("xtask.lines.oversized");
const OVER_BUDGET: DiagId = DiagId::new("xtask.size.over_budget");
const UNMEASURED: DiagId = DiagId::new("xtask.size.unmeasured");

const PROGRAM: Program<DiagCtxt> = Program {
    defaults: vun_command::undefaulted,
    configure: vun_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    build: None,
    root: Command::group("cargo xtask", "Repository automation.", task::COMMANDS),
};

fn run_corpus(_: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let root = repo_root();
    let corpus = Corpus::at(&root);
    let outcomes = corpus.outcomes()?;
    let mut tally = Tally::default();
    for o in &outcomes {
        tally.count(&o.verdict);
        shell.result(&Judged { path: o.path.display().to_string(), verdict: &o.verdict })?;
    }
    shell.result(&tally)?;
    let coverage = corpus.coverage()?;
    shell.result(&Document {
        kind: "coverage",
        name: vun_harness::ACCEPT.to_owned(),
        text: coverage.to_string(),
    })?;
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
        shell.status(&Passed::Lines { max: lines::MAX_LINES });
    }
    Ok(if oversized.is_empty() { Outcome::Success } else { Outcome::Failure })
}

fn check_layers(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    if Ranks::refresh(&repo_root())? {
        shell.status(&Refreshed);
    }
    let breaches = match Breach::all(&repo_root()) {
        Ok(breaches) => breaches,
        Err(why) => {
            let msg = Msg::new().text("cargo reports no metadata of the workspace");
            diag.error(Diagnostic::error(NO_METADATA, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    for breach in &breaches {
        diag.error(match breach {
            Breach::Upward { from, to } => {
                let msg = Msg::new()
                    .code(from.name.as_str())
                    .text(" in ")
                    .code(from.group.as_str())
                    .text(" depends on ")
                    .code(to.name.as_str())
                    .text(" in ")
                    .code(to.group.as_str())
                    .text(", a group not before its own");
                let help =
                    Msg::new().text("move the shared part into an earlier group, or own the trait there");
                Diagnostic::error(UPWARD, msg).child(ChildKind::Help, help)
            }
            Breach::Unranked(placed) => {
                let msg = Msg::new()
                    .code(placed.name.as_str())
                    .text(" is in the unranked group ")
                    .code(placed.group.as_str());
                let help = Msg::new().text("rank the group in `layers::RANKS` and spec 13, section 9");
                Diagnostic::error(UNRANKED, msg).child(ChildKind::Help, help)
            }
        });
    }
    if breaches.is_empty() {
        shell.status(&Passed::Layers { groups: layers::RANKS.iter().map(|r| r.len()).sum() });
    }
    Ok(if breaches.is_empty() { Outcome::Success } else { Outcome::Failure })
}

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
fn check_size(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let measured = match Measured::build(&repo_root()) {
        Ok(measured) => measured,
        Err(why) => {
            let msg = Msg::new().text("cannot measure the distributed ").code("vun");
            diag.error(Diagnostic::error(UNMEASURED, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let within = measured.bytes <= size::BUDGET;
    if within {
        let path = measured.path.display().to_string();
        shell.status(&Passed::Size { path, bytes: measured.bytes, budget: size::BUDGET });
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
