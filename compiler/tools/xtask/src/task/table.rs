//! Every task, and the commands of `cargo xtask` that run them.

use super::{Call, Task, WORDS, Work};

pub(crate) const FORMATTED: Task =
    Task::new("formatted", "fail if rustfmt would change a file", Work::Call(Call::Formatted));
pub(crate) const GENERATED: Task =
    Task::new("generated", "fail if a generated file is out of date", Work::Call(Call::Generated));
pub(crate) const GENERATE: Task =
    Task::new("generate", "write the generated files", Work::Call(Call::Generate));
pub(crate) const LINES: Task =
    Task::new("lines", "fail on source files too long to stay one concern", Work::Run(crate::check_lines));
pub(crate) const COMMENTS: Task = Task::new(
    "comments",
    "fail on more comment lines inside function bodies than the baseline of a group of crates holds",
    Work::Run(crate::comments::check),
)
.params(&[crate::comments::BLESS]);
pub(crate) const LAYERS: Task = Task::new(
    "layers",
    "fail on a crate depending on a later group of crates",
    Work::Run(crate::check_layers),
);
pub(crate) const NAMES: Task = Task::new(
    "names",
    "fail on a source file or directory named utils, helpers, common, misc, types, manager, service, handler or info",
    Work::Run(crate::check_names),
);
pub(crate) const DECISIONS: Task = Task::new(
    "decisions",
    "fail on a decision file that does not read, and write the index of the decisions",
    Work::Run(crate::check_decisions),
);
pub(crate) const CHECK: Task = Task::new(
    "check",
    "run the quick checks: formatting, generated sources, lines, comments, layers, names, decisions",
    Work::Needs,
)
.needs(&[&FORMATTED, &GENERATED, &LINES, &COMMENTS, &LAYERS, &NAMES, &DECISIONS]);
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
pub(crate) const RUNTIMES: Task = Task::new(
    "runtimes",
    "run the standard library's tests on every target this machine can run programs of",
    Work::Run(crate::runtimes::run),
);
pub(crate) const PACKAGE: Task = Task::new(
    "package",
    "pack a built vun and the library as this machine's release archive, its tree's hash beside it",
    Work::Run(crate::release::package),
)
.params(&[crate::release::BINARY, crate::release::OUT]);
pub(crate) const COMPONENTS: Task = Task::new(
    "components",
    "build the component libraries a release packs beside the toolchain, and the tests load",
    Work::Run(crate::release::build_components),
)
.params(&[crate::install::PROFILE, crate::release::TARGET, crate::release::TOOL]);
pub(crate) const RELEASE: Task = Task::new(
    "release",
    "write the release's manifest from the archives packed on each machine",
    Work::Run(crate::release::release),
)
.params(&[crate::release::OUT, crate::release::TAG]);
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
pub(crate) const PGO: Task = Task::new(
    "pgo",
    "record a `vun` running the profile workloads, the profile a `dist` build is optimized by",
    Work::Run(crate::pgo::pgo),
);
pub(crate) const INSTALL: Task = Task::new(
    "install",
    "build `vun` from this checkout and install it as the default toolchain",
    Work::Run(crate::install::install),
)
.params(&[crate::install::PROFILE]);

pub(crate) const TREE_SITTER: Task = Task::new(
    "tree-sitter",
    "generate the tree-sitter parser and compare its trees with the compiler's on every source",
    Work::Run(crate::tree_sitter::check),
);
pub(crate) const TREE_SITTER_PUBLISH: Task = Task::new(
    "tree-sitter-publish",
    "publish the tree-sitter grammar to its repository, record the commit, and generate the editors' packages",
    Work::Run(crate::tree_sitter::publish::publish),
)
.needs(&[&GENERATED, &TREE_SITTER])
.params(&[crate::tree_sitter::publish::MESSAGE, crate::tree_sitter::publish::REPO]);

/// The tasks `cargo xtask` offers as commands; the others are reached through them.
macro_rules! commands {
    ($($task:path),* $(,)?) => {
        &[$(vun_command::Command::item($task.name, $task.about, |d, a, s| $task.run(d, a, s)).params($task.params)),*]
    };
}

pub(crate) const COMMANDS: &[vun_command::Command<vun_diag::DiagCtxt>] = commands![
    CHECK,
    COMMENTS,
    CORPUS,
    DECISIONS,
    HOOKS,
    INSTALL,
    LAYERS,
    LINES,
    NAMES,
    COMPONENTS,
    PACKAGE,
    PGO,
    PUSH,
    RELEASE,
    RUNTIMES,
    TREE_SITTER,
    TREE_SITTER_PUBLISH,
    TYPESET
];
