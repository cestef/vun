//! `cargo xtask comments`: a function body holds no comment lines, as the code rules say. The
//! lines that exist are counted per group of crates and may only fall: the count of a group
//! above its baseline fails, and below it the baseline is lowered with `--bless`.

mod baseline;
mod count;
mod drift;
#[cfg(test)]
mod tests;

use std::io;

use vun_command::{Args, Outcome, Param, Shell};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;

use crate::report::Passed;
pub(crate) use count::Counts;
use drift::Drift;

pub(crate) const BLESS: Param = Param::flag("bless").about("write the counts as the baseline");

const ABOVE: DiagId = DiagId::new("xtask.comments.above");
const BELOW: DiagId = DiagId::new("xtask.comments.below");
const UNCOUNTED: DiagId = DiagId::new("xtask.comments.uncounted");
const NO_BASELINE: DiagId = DiagId::new("xtask.comments.no_baseline");

impl Drift {
    fn diagnostic(&self) -> Diagnostic {
        let found = Msg::new().code(self.group.as_str()).text(" has ").num(self.counted);
        let found = found.text(" comment lines inside function bodies, ");
        if self.above() {
            let msg = found.text("more than the baseline's ").num(self.allowed);
            let help =
                Msg::new().text("remove the comments: name what the code says, or move it to a doc comment");
            Diagnostic::error(ABOVE, msg).child(ChildKind::Help, help)
        } else {
            let msg = found.text("fewer than the baseline's ").num(self.allowed);
            let help = Msg::new().text("lower the baseline: run ").code("cargo xtask comments --bless");
            Diagnostic::error(BELOW, msg).child(ChildKind::Help, help)
        }
    }
}

#[expect(clippy::unnecessary_wraps, reason = "every task has the `Run` signature")]
pub(crate) fn check(diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let root = repo_root();
    let counts = match Counts::of(&root) {
        Ok(counts) => counts,
        Err(why) => {
            diag.error(
                Diagnostic::error(UNCOUNTED, Msg::new().text("the comments are not counted")).caused_by(&why),
            );
            return Ok(Outcome::Failure);
        }
    };
    if args.flag(&BLESS) {
        return Ok(match counts.bless(&root) {
            Ok(()) => Outcome::Success,
            Err(why) => {
                diag.error(
                    Diagnostic::error(NO_BASELINE, Msg::new().text("the baseline is not written"))
                        .caused_by(&why),
                );
                Outcome::Failure
            }
        });
    }
    let baseline = match Counts::baseline(&root) {
        Ok(baseline) => baseline,
        Err(why) => {
            let msg = Msg::new().text("the baseline is not read");
            let help = Msg::new().text("write it with ").code("cargo xtask comments --bless");
            diag.error(Diagnostic::error(NO_BASELINE, msg).caused_by(&why).child(ChildKind::Help, help));
            return Ok(Outcome::Failure);
        }
    };
    let drifts = counts.drift(&baseline);
    for drift in &drifts {
        diag.error(drift.diagnostic());
    }
    if drifts.is_empty() {
        shell.status(&Passed::Comments { groups: counts.0.len(), lines: counts.total() });
    }
    Ok(if drifts.is_empty() { Outcome::Success } else { Outcome::Failure })
}
