use super::*;
use crate::{Action, Outcome};

const FILE: Param = Param { name: "file", short: None, about: "a source file", kind: ParamKind::Positional };
const FORMAT: Param =
    Param { name: "format", short: Some('f'), about: "output format", kind: ParamKind::Option };
const QUIET: Param = Param { name: "quiet", short: Some('q'), about: "print less", kind: ParamKind::Flag };
const COLOR: Param = Param { name: "color", short: None, about: "use colors", kind: ParamKind::Flag };

fn ok(_: &mut (), _: &Args, out: &mut dyn std::io::Write) -> std::io::Result<Outcome> {
    out.flush().map(|()| Outcome::Success)
}

const PROGRAM: Program<()> = Program {
    configure: crate::unconfigured,
    version: "1.0",
    root: Command {
        name: "tool",
        about: "a tool",
        params: &[COLOR],
        action: Action::Group(&[
            Command {
                name: "dump",
                about: "print things",
                params: &[],
                action: Action::Group(&[Command {
                    name: "cst",
                    about: "the tree",
                    params: &[FILE, FORMAT, QUIET],
                    action: Action::Run(ok),
                }]),
            },
            Command { name: "build", about: "build it", params: &[], action: Action::Planned },
        ]),
    },
};

fn argv(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn names(selected: &Selected<'_, ()>) -> Vec<&'static str> {
    selected.iter().map(|c| c.name).collect()
}

fn error(line: &str) -> (DiagId, String) {
    match PROGRAM.parse(&argv(line)) {
        Err(d) => (d.id, d.children.iter().map(|c| c.msg.to_string()).collect()),
        Ok(_) => panic!("{line:?} parsed"),
    }
}

#[test]
fn runs_with_typed_arguments() {
    let Ok(Request::Run(path, args)) = PROGRAM.parse(&argv("dump cst -q a.ov --format=json")) else {
        panic!("not a run")
    };
    assert_eq!(names(&path), ["tool", "dump", "cst"]);
    assert_eq!(
        (args.value(&FILE), args.value(&FORMAT), args.flag(&QUIET)),
        (Some("a.ov"), Some("json"), true)
    );
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("dump cst --format short a.ov")) else {
        panic!("not a run")
    };
    assert_eq!(args.value(&FORMAT), Some("short"));
}

#[test]
fn help_and_version() {
    for line in ["", "dump", "help dump", "dump cst --help", "dump -h cst"] {
        assert!(matches!(PROGRAM.parse(&argv(line)), Ok(Request::Help(_))), "{line:?}");
    }
    assert!(matches!(PROGRAM.parse(&argv("-V")), Ok(Request::Version)));
    let page = Selected::root(&PROGRAM.root).help(&PROGRAM);
    assert!(page.starts_with("tool 1.0\na tool\n\nusage: tool <command> [options]"), "{page}");
    assert!(page.contains("build  build it (planned)"), "{page}");
}

#[test]
fn usage_errors_suggest_and_point_at_help() {
    assert_eq!(error("dmp"), (errors::CLI_UNKNOWN_COMMAND, "did you mean `dump`?".to_owned()));
    assert_eq!(
        error("dump cst --quite a"),
        (errors::CLI_UNKNOWN_OPTION, "did you mean `--quiet`?".to_owned())
    );
    assert_eq!(
        error("dump cst"),
        (errors::CLI_MISSING_ARGUMENT, "usage: `tool dump cst [options] <file>`".to_owned())
    );
    assert_eq!(
        error("dump cst a b"),
        (errors::CLI_UNEXPECTED_ARGUMENT, "see `tool dump cst --help`".to_owned())
    );
    assert_eq!(error("dump cst a --format").0, errors::CLI_MISSING_VALUE);
    assert_eq!(error("-V2").0, errors::CLI_UNKNOWN_OPTION);
    assert!(matches!(PROGRAM.parse(&argv("-help")), Ok(Request::Help(_))));
}

#[test]
fn group_options_apply_below() {
    for line in ["--color dump cst a.ov", "dump --color cst a.ov", "dump cst a.ov --color"] {
        let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv(line)) else { panic!("{line:?} did not run") };
        assert!(args.flag(&COLOR), "{line:?}");
    }
}
