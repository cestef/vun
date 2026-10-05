//! `cargo xtask runtimes`: the tests of the standard library, run on every target this machine can
//! run programs of. Each runtime is its own code (`std.os.linux.kernel`, `std.os.libc`,
//! `std.os.wasi`, `std.os.windows`), so the tests that pass on one prove nothing of another; which
//! targets run here is the machine's tools file (`vun/tools.vun`: a `.runner` for a
//! target that is not the machine's own).

use std::io;
use std::path::Path;
use std::process::{Command, Output};

use vun_command::{Args, Outcome, Shell};
use vun_data::Object;
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_term::{Record, Status, Style, Text, Verb};

use crate::cargo;

const UNBUILT: DiagId = DiagId::new("xtask.runtimes.unbuilt");
const UNLISTED: DiagId = DiagId::new("xtask.runtimes.unlisted");

/// The package whose tests run.
const PACKAGE: &str = "lib/std";

/// A target that runs here, and whether its tests passed.
struct Tested {
    target: String,
    passed: bool,
    /// The line the test run ends with.
    tally: String,
}

impl Record for Tested {
    fn kind(&self) -> &'static str {
        "runtime"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.target.clone()).plain(format!(" {}", self.tally))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("target", self.target.clone())
            .with("passed", self.passed)
            .with("tally", self.tally.clone())
    }
}

impl Status for Tested {
    fn verb(&self) -> Verb {
        if self.passed { Verb::good("Passed") } else { Verb::failed("Failed") }
    }
}

/// The targets `vun target list` says run on this machine and have what they need.
fn runnable(vun: &Path, root: &Path) -> io::Result<Vec<String>> {
    let listed = Command::new(vun).args(["--format", "json", "target", "list"]).current_dir(root).output()?;
    let text = String::from_utf8_lossy(&listed.stdout);
    let mut targets = Vec::new();
    for line in text.lines() {
        let Ok(row) = vun_data::json::parse(line) else { continue };
        let field = |name| row.get(name);
        // Where it runs is `here`, or the runner the tools file names; `elsewhere` is nowhere.
        let runs = field("runs").and_then(|v| v.as_str()).is_some_and(|runs| runs != "elsewhere");
        let ready = field("ready").and_then(vun_data::Value::as_bool) == Some(true);
        if let (true, true, Some(target)) = (runs, ready, field("target").and_then(|v| v.as_str())) {
            targets.push(target.to_owned());
        }
    }
    Ok(targets)
}

/// The line of `output` the test run ends with, `Tested 190 passed, ...`, or its last line.
fn tally(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stdout);
    let text = if text.trim().is_empty() { String::from_utf8_lossy(&output.stderr) } else { text };
    let tested = text.lines().rev().find(|l| l.trim_start().starts_with("Tested"));
    let last = text.lines().rev().find(|l| !l.trim().is_empty());
    tested.or(last).map_or_else(String::new, |l| l.trim().to_owned())
}

pub(crate) fn run(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let root = repo_root();
    let vun = match cargo::vun(&root, "dev") {
        Ok(vun) => vun,
        Err(why) => {
            let msg = Msg::new().text("cannot build ").code("vun");
            diag.error(Diagnostic::error(UNBUILT, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let targets = match runnable(&vun, &root) {
        Ok(targets) => targets,
        Err(why) => {
            let msg = Msg::new().text("cannot list the targets with ").code("vun target list");
            diag.error(Diagnostic::error(UNLISTED, msg).caused_by(&why));
            return Ok(Outcome::Failure);
        }
    };
    let mut outcome = Outcome::Success;
    for target in targets {
        let ran =
            Command::new(&vun).args(["test", "--target", &target, PACKAGE]).current_dir(&root).output()?;
        let passed = ran.status.success();
        if !passed {
            outcome = Outcome::Failure;
        }
        shell.status(&Tested { target, passed, tally: tally(&ran) });
    }
    Ok(outcome)
}
