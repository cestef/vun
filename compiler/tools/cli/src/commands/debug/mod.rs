//! `vun debug`: low-level commands for developing the compiler: each representation of a file
//! with origins (`vun debug dump`), how far the stages take a file, how the backends agree on a program, and a
//! failing file reduced.

mod compare;
mod coverage;
mod disasm;
mod dump;
mod queries;
mod reduce;
mod smith;
mod workers;

use vun_command::Command;

use super::FILE;
use dump::{DUMP, view};
use vun_driver::Session;

pub(super) const GROUP: &str = "debug";

/// Each view of a file, as `vun debug dump <view>` prints it.
const DUMPS: &[Command<Session>] = &[
    view!(Tokens),
    view!(Cst),
    view!(Manifest),
    view!(Plan),
    view!(Hir),
    view!(Types, dump::AT),
    view!(Values, dump::AT),
    view!(Mir, dump::AT),
    view!(Layout),
    view!(Program),
    view!(Lir),
    view!(Code, vun_driver::codegen::BACKEND),
    view!(Frames, vun_driver::codegen::BACKEND),
];

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::group(DUMP, "each representation of a file as text, with origins", DUMPS),
    Command::run(
        "coverage",
        "how far MIR and LIR take the functions of a file, and where each stops",
        |s, args, shell| shell.document(DUMP, "coverage".to_owned(), |out| coverage::coverage(s, args, out)),
    )
    .params(&[FILE]),
    Command::run("disasm", "the code of an ELF file read back as instructions", disasm::disasm)
        .params(&[disasm::OBJECT, disasm::SYMBOL])
        .examples(disasm::EXAMPLES),
    Command::run(
        "queries",
        "what the queries saved beside a root file hold, largest first",
        queries::queries,
    )
    .params(&[FILE]),
    Command::run(
        "compare",
        "build and run a program through every backend that generates code, and compare",
        compare::compare,
    )
    .params(compare::PARAMS),
    Command::run(
        "reduce",
        "the least text that fails as a file does: the same crash, or the same error",
        reduce::reduce,
    )
    .params(&[FILE, reduce::KEEP, vun_driver::codegen::BACKEND]),
    Command::run(
        "smith",
        "programs made by chance, each tried through every stage and backend",
        smith::smith,
    )
    .params(smith::PARAMS),
];
