//! `vun debug`: low-level commands for developing the compiler: each representation of a file
//! with origins, how far the stages take a file, and how the backends agree on a program.

mod compare;
mod coverage;
mod dump;
mod types;

use vun_command::Command;

use super::FILE;
use crate::Session;
use dump::Stage;

pub(super) const GROUP: &str = "debug";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run("tokens", "tokens and trivia with byte ranges", |s, args, shell| {
        Stage::Tokens.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run("cst", "the concrete syntax tree, one node per line", |s, args, shell| {
        Stage::Cst.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run("manifest", "the package manifest read from a build.vun", |s, args, shell| {
        Stage::Manifest.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run("plan", "the steps a build.vun's build records, one per line", |s, args, shell| {
        Stage::Plan.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run("hir", "the high-level representation, names resolved, with origins", |s, args, shell| {
        Stage::Hir.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run(
        "types",
        "what typing found in each function, plain and in each instantiation reached",
        types::types,
    )
    .params(&[FILE]),
    Command::run("mir", "the mid-level representation of each function, with origins", |s, args, shell| {
        Stage::Mir.dump(s, args, shell)
    })
    .params(&[FILE]),
    Command::run(
        "layout",
        "the layout of each type the file declares: sizes, offsets, tags and niches",
        |s, args, shell| Stage::Layout.dump(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        "program",
        "the exports of a root file and the MIR of every function they reach",
        |s, args, shell| Stage::Program.dump(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        "lir",
        "the low-level representation of the program a root file exports, with origins",
        |s, args, shell| Stage::Lir.dump(s, args, shell),
    )
    .params(&[FILE]),
    Command::run(
        "code",
        "what a backend generates for the program a root file exports, when it is text",
        |s, args, shell| Stage::Code.dump(s, args, shell),
    )
    .params(&[FILE, crate::codegen::BACKEND]),
    Command::run(
        "frames",
        "the frame size of each function of the program a root file exports, as its tools report them",
        |s, args, shell| Stage::Frames.dump(s, args, shell),
    )
    .params(&[FILE, crate::codegen::BACKEND]),
    Command::run(
        "coverage",
        "how far MIR and LIR take the functions of a file, and where each stops",
        coverage::coverage,
    )
    .params(&[FILE]),
    Command::run(
        "compare",
        "build and run a program through every backend that generates code, and compare",
        compare::compare,
    )
    .params(&[FILE]),
];
