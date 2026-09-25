//! `vun debug`: low-level commands for developing the compiler: each representation of a file
//! with origins, how far the stages take a file, and how the backends agree on a program.

mod compare;
mod coverage;
mod dump;

use vun_command::{Action, Command};

use super::FILE;
use crate::Session;
use dump::Stage;

pub(super) const GROUP: &str = "debug";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command {
        name: "tokens",
        about: "tokens and trivia with byte ranges",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Tokens.dump(s, args, out)),
    },
    Command {
        name: "cst",
        about: "the concrete syntax tree, one node per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Cst.dump(s, args, out)),
    },
    Command {
        name: "manifest",
        about: "the package manifest read from a build.vun",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Manifest.dump(s, args, out)),
    },
    Command {
        name: "plan",
        about: "the steps a build.vun's build records, one per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Plan.dump(s, args, out)),
    },
    Command {
        name: "hir",
        about: "the high-level representation, names resolved, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Hir.dump(s, args, out)),
    },
    Command {
        name: "mir",
        about: "the mid-level representation of each function, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Mir.dump(s, args, out)),
    },
    Command {
        name: "layout",
        about: "the layout of each type the file declares: sizes, offsets, tags and niches",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Layout.dump(s, args, out)),
    },
    Command {
        name: "program",
        about: "the exports of a root file and the MIR of every function they reach",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Program.dump(s, args, out)),
    },
    Command {
        name: "lir",
        about: "the low-level representation of the program a root file exports, with origins",
        params: &[FILE],
        action: Action::Run(|s, args, out| Stage::Lir.dump(s, args, out)),
    },
    Command {
        name: "code",
        about: "what a backend generates for the program a root file exports, when it is text",
        params: &[FILE, crate::codegen::BACKEND],
        action: Action::Run(|s, args, out| Stage::Code.dump(s, args, out)),
    },
    Command {
        name: "frames",
        about: "the frame size of each function of the program a root file exports, as its tools report them",
        params: &[FILE, crate::codegen::BACKEND],
        action: Action::Run(|s, args, out| Stage::Frames.dump(s, args, out)),
    },
    Command::planned("link", "the link plan"),
    Command {
        name: "coverage",
        about: "how far MIR and LIR take the functions of a file, and where each stops",
        params: &[FILE],
        action: Action::Run(coverage::coverage),
    },
    Command {
        name: "compare",
        about: "build and run a program through every backend that generates code, and compare",
        params: &[FILE],
        action: Action::Run(compare::compare),
    },
];
