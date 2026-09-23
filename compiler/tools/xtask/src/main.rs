//! Repository automation, run through the `cargo xtask` alias.

use std::process::ExitCode;

struct Task {
    name: &'static str,
    about: &'static str,
}

const TASKS: &[Task] = &[
    Task { name: "grammar", about: "generate overt-grammar from lib/lang/lang/syntax/grammar.ov" },
    Task { name: "corpus", about: "run corpus/accept and corpus/reject through the harness" },
];

fn main() -> ExitCode {
    let requested = std::env::args().nth(1);
    if let Some(task) = TASKS.iter().find(|t| Some(t.name) == requested.as_deref()) {
        eprintln!("xtask {}: not available yet ({})", task.name, task.about);
    } else {
        eprintln!("usage: cargo xtask <task>");
        for t in TASKS {
            eprintln!("  {:<8}  {}", t.name, t.about);
        }
    }
    ExitCode::FAILURE
}
