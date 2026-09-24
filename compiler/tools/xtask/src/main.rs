//! Repository automation, run through the `cargo xtask` alias.

mod corpus;
mod explanations;
mod lines;
mod size;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::process::ExitCode;

use vun_command::{Action, Args, Command, Outcome, Program};
use vun_diag::{ChildKind, DiagCtxt, DiagId, DiagSink, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_origin::OriginTable;
use vun_render_short::ShortRenderer;
use vun_span::SourceFiles;

use corpus::Corpus;
use lines::Oversized;
use size::Measured;

/// What tasks report to.
#[derive(Default)]
struct Cx {
    diag: DiagCtxt,
}

const OVERSIZED: DiagId = DiagId::new("xtask.lines.oversized");
const OVER_BUDGET: DiagId = DiagId::new("xtask.size.over_budget");

const PROGRAM: Program<Cx> = Program {
    configure: vun_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    root: Command {
        name: "cargo xtask",
        about: "Repository automation.",
        params: &[],
        action: Action::Group(&[
            Command {
                name: "corpus",
                about: "run corpus/accept and corpus/reject through the stages that exist",
                params: &[],
                action: Action::Run(run_corpus),
            },
            Command {
                name: "lines",
                about: "fail on source files too long to stay one concern",
                params: &[],
                action: Action::Run(check_lines),
            },
            Command {
                name: "size",
                about: "fail when the distributed vun outgrows its budget",
                params: &[],
                action: Action::Run(check_size),
            },
        ]),
    },
};

fn run_corpus(_: &mut Cx, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let outcomes = Corpus { root: &repo_root() }.outcomes()?;
    let (mut passed, mut pending, mut planned, mut shown, mut failed) = (0, 0, 0, 0, 0);
    for o in &outcomes {
        match &o.verdict {
            corpus::Verdict::Pass => passed += 1,
            corpus::Verdict::Pending(later) => {
                pending += 1;
                let ids: Vec<_> = later.iter().map(|at| format!("{}:{}", at.line, at.id)).collect();
                writeln!(out, "pending {}: awaits {}", o.path.display(), ids.join(", "))?;
            }
            corpus::Verdict::Shown => shown += 1,
            corpus::Verdict::Planned(waits) => {
                planned += 1;
                writeln!(out, "planned {}: waits for", o.path.display())?;
                for r in waits {
                    writeln!(out, "  {}: {}", r.at.line, r.text.replace('\n', " | "))?;
                }
            }
            corpus::Verdict::Ready => {
                failed += 1;
                writeln!(
                    out,
                    "fail    {}: checks clean; move it to {}",
                    o.path.display(),
                    vun_harness::ACCEPT
                )?;
            }
            corpus::Verdict::Unexplained(why) => {
                failed += 1;
                writeln!(out, "fail    {}: {why}", o.path.display())?;
            }
            corpus::Verdict::Malformed(malformed) => {
                failed += 1;
                writeln!(out, "fail    {}: malformed annotation: {malformed}", o.path.display())?;
            }
            corpus::Verdict::Undeclared(ids) => {
                failed += 1;
                writeln!(out, "fail    {}", o.path.display())?;
                for at in ids {
                    writeln!(
                        out,
                        "  undeclared {}:{} (declare it in lib/lang/lang/errors.vun)",
                        at.line, at.id
                    )?;
                }
            }
            corpus::Verdict::Fail(mismatch) => {
                failed += 1;
                writeln!(out, "fail    {}", o.path.display())?;
                for at in &mismatch.missing {
                    writeln!(out, "  missing    {}:{}", at.line, at.id)?;
                }
                for at in &mismatch.unexpected {
                    writeln!(out, "  unexpected {}:{}", at.line, at.id)?;
                }
                for (expected, found) in &mismatch.differing {
                    writeln!(
                        out,
                        "  differing  {}:{} expected {:?}, found:",
                        expected.at.line, expected.at.id, expected
                    )?;
                    for r in found {
                        writeln!(out, "    col {}: {}", r.col, r.text.replace('\n', " | "))?;
                    }
                }
            }
        }
    }
    writeln!(out, "{passed} passed, {pending} pending, {planned} planned, {shown} shown, {failed} failed")?;
    writeln!(out, "corpus/accept: {}", Corpus { root: &repo_root() }.coverage()?)?;
    Ok(if failed == 0 { Outcome::Success } else { Outcome::Failure })
}

fn check_lines(cx: &mut Cx, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let oversized = Oversized::all(&repo_root())?;
    for o in &oversized {
        let msg = Msg::new()
            .code(o.path.display().to_string())
            .text(" has ")
            .num(o.lines)
            .text(" lines, more than ")
            .num(lines::MAX_LINES);
        let help = Msg::new().text("split it into a directory module, one concern per file");
        cx.diag.error(Diagnostic::error(OVERSIZED, msg).child(ChildKind::Help, help));
    }
    if oversized.is_empty() {
        writeln!(out, "every source file has at most {} lines", lines::MAX_LINES)?;
    }
    Ok(if oversized.is_empty() { Outcome::Success } else { Outcome::Failure })
}

fn check_size(cx: &mut Cx, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let measured = Measured::build(&repo_root())?;
    let within = measured.bytes <= size::BUDGET;
    if within {
        writeln!(
            out,
            "{} is {} bytes, {} under the budget of {}",
            measured.path.display(),
            measured.bytes,
            size::BUDGET - measured.bytes,
            size::BUDGET
        )?;
    } else {
        let msg = Msg::new()
            .code(measured.path.display().to_string())
            .text(" is ")
            .num(measured.bytes)
            .text(" bytes, over the budget of ")
            .num(size::BUDGET);
        let help = Msg::new().text("find what grew; raising the budget takes a decision (decision 131)");
        cx.diag.error(Diagnostic::error(OVER_BUDGET, msg).child(ChildKind::Help, help));
    }
    Ok(if within { Outcome::Success } else { Outcome::Failure })
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut cx = Cx::default();
    let outcome = PROGRAM.run(&argv, &mut cx, &mut io::stdout().lock()).unwrap_or_else(|refusal| {
        cx.diag.error(*refusal.diagnostic);
        refusal.outcome
    });
    let (origins, sources) = (OriginTable::new(), SourceFiles::new());
    let mut renderer = ShortRenderer::new(io::stderr());
    for d in cx.diag.sorted() {
        renderer.emit(d, &origins, &sources);
    }
    outcome.into()
}
