//! `vun debug`: low-level commands for developing the compiler: each representation of a file
//! with origins, how far the stages take a file, how the backends agree on a program, and a
//! failing file reduced.

mod compare;
mod coverage;
mod dump;
mod queries;
mod reduce;
mod types;

use vun_command::Command;

use super::FILE;
use dump::{DUMP, Stage};
use vun_driver::Session;

pub(super) const GROUP: &str = "debug";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run(Stage::Tokens.name(), "tokens and trivia with byte ranges", |s, args, shell| {
        Stage::Tokens.show(s, args, shell)
    })
    .params(&[FILE]),
    Command::run(Stage::Cst.name(), "the concrete syntax tree, one node per line", |s, args, shell| {
        Stage::Cst.show(s, args, shell)
    })
    .params(&[FILE]),
    Command::run(Stage::Manifest.name(), "the package manifest read from a build.vun", |s, args, shell| {
        Stage::Manifest.show(s, args, shell)
    })
    .params(&[FILE]),
    Command::run(
        Stage::Plan.name(),
        "the steps a build.vun's build records, one per line",
        |s, args, shell| Stage::Plan.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Hir.name(),
        "the high-level representation, names resolved, with origins",
        |s, args, shell| Stage::Hir.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        "types",
        "what typing found in each function, plain and in each instantiation reached",
        |s, args, shell| shell.document(DUMP, "types".to_owned(), |out| types::types(s, args, out)),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Mir.name(),
        "the mid-level representation of each function, with origins",
        |s, args, shell| Stage::Mir.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Layout.name(),
        "the layout of each type the file declares: sizes, offsets, tags and niches",
        |s, args, shell| Stage::Layout.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Program.name(),
        "the exports of a root file and the MIR of every function they reach",
        |s, args, shell| Stage::Program.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Lir.name(),
        "the low-level representation of the program a root file exports, with origins",
        |s, args, shell| Stage::Lir.show(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        Stage::Code.name(),
        "what a backend generates for the program a root file exports, when it is text",
        |s, args, shell| Stage::Code.show(s, args, shell),
    )
    .params(&[FILE, vun_driver::codegen::BACKEND]),
    Command::run(
        Stage::Frames.name(),
        "the frame size of each function of the program a root file exports, as its tools report them",
        |s, args, shell| Stage::Frames.show(s, args, shell),
    )
    .params(&[FILE, vun_driver::codegen::BACKEND]),
    Command::run(
        "coverage",
        "how far MIR and LIR take the functions of a file, and where each stops",
        |s, args, shell| shell.document(DUMP, "coverage".to_owned(), |out| coverage::coverage(s, args, out)),
    )
    .params(&[FILE]),
    Command::run(
        "queries",
        "what the queries saved beside a root file hold, largest first",
        queries::queries,
    )
    .params(&[FILE]),
    Command::run(
        "compare",
        "build and run a program through every backend that generates code, and compare",
        |s, args, shell| shell.document(DUMP, "compare".to_owned(), |out| compare::compare(s, args, out)),
    )
    .params(&[FILE]),
    Command::run(
        "reduce",
        "the least text that fails as a file does: the same crash, or the same error",
        reduce::reduce,
    )
    .params(&[FILE, reduce::KEEP, vun_driver::codegen::BACKEND]),
];
