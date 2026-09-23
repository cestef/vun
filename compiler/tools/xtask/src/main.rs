//! Repository automation, run through the `cargo xtask` alias.

mod corpus;
mod errors;
mod explanations;
mod generate;
mod grammar;
mod lines;
mod names;
mod primitives;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::process::ExitCode;

use overt_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};
use overt_diag::{ChildKind, DiagCtxt, DiagId, DiagSink, Diagnostic, Msg};
use overt_harness::repo_root;
use overt_origin::OriginTable;
use overt_render_short::ShortRenderer;
use overt_span::SourceFiles;

/// What tasks report to.
#[derive(Default)]
struct Cx {
    diag: DiagCtxt,
}

const OVERSIZED: DiagId = DiagId::new("xtask.lines.oversized");
const STALE: DiagId = DiagId::new("xtask.generate.stale");

const CHECK: Param = Param {
    name: "check",
    short: None,
    about: "fail if a generated file is out of date instead of writing it",
    kind: ParamKind::Flag,
};

const PROGRAM: Program<Cx> = Program {
    configure: overt_command::unconfigured,
    version: env!("CARGO_PKG_VERSION"),
    root: Command {
        name: "cargo xtask",
        about: "Repository automation.",
        params: &[],
        action: Action::Group(&[
            Command {
                name: "generate",
                about: "regenerate Rust sources from their Overt sources (grammar, diagnostic ids)",
                params: &[CHECK],
                action: Action::Run(generate_all),
            },
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
        ]),
    },
};

fn generate_all(cx: &mut Cx, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let root = repo_root();
    let mut outcome = Outcome::Success;
    for generator in generate::ALL {
        let generated = match generator.generate(&root) {
            Ok(generated) => generated,
            Err(d) => {
                cx.diag.error(*d);
                outcome = Outcome::Failure;
                continue;
            }
        };
        let path = root.join(generator.output);
        if std::fs::read_to_string(&path).is_ok_and(|current| current == generated) {
            writeln!(out, "{} is up to date", generator.output)?;
        } else if args.flag(&CHECK) {
            let msg = Msg::new().code(generator.output).text(" is out of date with ").code(generator.source);
            let help = Msg::new().text("run ").code("cargo xtask generate");
            cx.diag.error(Diagnostic::error(STALE, msg).child(ChildKind::Help, help));
            outcome = Outcome::Failure;
        } else {
            std::fs::write(&path, generated)?;
            writeln!(out, "wrote {}", generator.output)?;
        }
    }
    Ok(outcome)
}

fn run_corpus(_: &mut Cx, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let outcomes = corpus::run(&repo_root())?;
    let (mut passed, mut pending, mut shown, mut failed) = (0, 0, 0, 0);
    for o in &outcomes {
        match &o.verdict {
            corpus::Verdict::Pass => passed += 1,
            corpus::Verdict::Pending(later) => {
                pending += 1;
                let ids: Vec<_> = later.iter().map(|at| format!("{}:{}", at.line, at.id)).collect();
                writeln!(out, "pending {}: awaits {}", o.path.display(), ids.join(", "))?;
            }
            corpus::Verdict::Shown => shown += 1,
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
                        "  undeclared {}:{} (declare it in lib/lang/lang/errors.ov)",
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
    writeln!(out, "{passed} passed, {pending} pending, {shown} shown, {failed} failed")?;
    Ok(if failed == 0 { Outcome::Success } else { Outcome::Failure })
}

fn check_lines(cx: &mut Cx, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let oversized = lines::oversized(&repo_root())?;
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

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut cx = Cx::default();
    let outcome =
        overt_command::run(&PROGRAM, &argv, &mut cx, &mut io::stdout().lock()).unwrap_or_else(|refusal| {
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
