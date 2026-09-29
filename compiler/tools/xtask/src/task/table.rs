//! Every task, and the commands of `cargo xtask` that run them.

use super::{Call, Task, WORDS, Work};

pub(crate) const FORMATTED: Task =
    Task::new("formatted", "fail if rustfmt would change a file", Work::Call(Call::Formatted));
pub(crate) const GENERATED: Task =
    Task::new("generated", "fail if a generated file is out of date", Work::Call(Call::Generated));
pub(crate) const LINES: Task =
    Task::new("lines", "fail on source files too long to stay one concern", Work::Run(crate::check_lines));
pub(crate) const LAYERS: Task = Task::new(
    "layers",
    "fail on a crate depending on a later group of crates",
    Work::Run(crate::check_layers),
);
pub(crate) const CHECK: Task =
    Task::new("check", "run the quick checks: formatting, generated sources, lines, layers", Work::Needs)
        .needs(&[&FORMATTED, &GENERATED, &LINES, &LAYERS]);
pub(crate) const PUSH: Task = Task::new(
    "push",
    "run the quick checks, then `jj git push` with the words given",
    Work::Call(Call::Push),
)
.needs(&[&CHECK])
.params(&[WORDS]);
pub(crate) const CORPUS: Task = Task::new(
    "corpus",
    "run corpus/accept and corpus/reject through the stages that exist",
    Work::Run(crate::run_corpus),
);
pub(crate) const SIZE: Task =
    Task::new("size", "fail when the distributed vun outgrows its budget", Work::Run(crate::check_size));
pub(crate) const TYPESET: Task = Task::new(
    "typeset",
    "compile the whitepaper and the README code card with typst",
    Work::Run(crate::typeset::compile_all),
);
pub(crate) const HOOKS: Task = Task::new(
    "hooks",
    "set this repository's jj aliases that run tasks: `jj check`, `jj push`",
    Work::Run(crate::hooks::install_all),
);

/// The tasks `cargo xtask` offers as commands; the others are reached through them.
macro_rules! commands {
    ($($task:path),* $(,)?) => {
        &[$(vun_command::Command::run($task.name, $task.about, |d, a, s| $task.run(d, a, s)).params($task.params)),*]
    };
}

pub(crate) const COMMANDS: &[vun_command::Command<vun_diag::DiagCtxt>] =
    commands![CHECK, CORPUS, HOOKS, LAYERS, LINES, PUSH, SIZE, TYPESET];
