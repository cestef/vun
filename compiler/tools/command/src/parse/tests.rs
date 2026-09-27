use super::*;
use crate::{Dialect, Outcome, PathKind, Shell, Values};
use std::io::Write;

const FILE: Param = Param::positional("file").about("a source file").values(Values::Path(PathKind::File));
const FORMAT: Param = Param::option("format")
    .short('o')
    .about("output format")
    .values(Values::Choice(|| vec!["json", "short"]));
const FAST: Param = Param::flag("fast").short('f').about("go fast");
const DEEP: Param = Param::flag("deep").about("look deeper");
const REST: Param = Param::positional("rest").arity(Arity::Many);
/// Named as the root's `--version` flag, but an option of its command.
const RELEASE: Param = Param::option("version").about("which release");

fn ok(_: &mut (), _: &Args, shell: &mut Shell<'_>) -> std::io::Result<Outcome> {
    shell.out.flush().map(|()| Outcome::Success)
}

const PROGRAM: Program<()> = Program {
    configure: crate::unconfigured,
    version: "1.0",
    root: Command::group(
        "tool",
        "a tool",
        &[
            Command::group(
                "dump",
                "print things",
                &[Command::run("cst", "the tree", ok).params(&[FILE, FORMAT, FAST])],
            ),
            Command::run("exec", "run it", ok).params(&[FILE, RELEASE, REST]).aliases(&["x"]),
        ],
    )
    .params(&[DEEP]),
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
    let Ok(Request::Run(path, args)) = PROGRAM.parse(&argv("dump cst -f a.vun --format=json")) else {
        panic!("not a run")
    };
    assert_eq!(names(&path), ["tool", "dump", "cst"]);
    assert_eq!(
        (args.value(&FILE), args.value(&FORMAT), args.flag(&FAST)),
        (Some("a.vun"), Some("json"), true)
    );
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("dump cst --format short a.vun")) else {
        panic!("not a run")
    };
    assert_eq!(args.value(&FORMAT), Some("short"));
}

#[test]
fn help_and_version() {
    for line in ["", "dump", "help dump", "dump cst --help", "dump -h cst"] {
        assert!(matches!(PROGRAM.parse(&argv(line)), Ok(Request::Help(..))), "{line:?}");
    }
    assert!(matches!(PROGRAM.parse(&argv("-V")), Ok(Request::Version)));
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("exec a.vun --version 2")) else {
        panic!("a command's own `--version` is not the root's")
    };
    assert_eq!(args.value(&RELEASE), Some("2"));
    let detail = |line| match PROGRAM.parse(&argv(line)) {
        Ok(Request::Help(_, _, detail)) => detail,
        _ => panic!("{line:?} is not help"),
    };
    assert_eq!(
        (detail("dump -h"), detail("dump --help"), detail("help dump")),
        (Detail::Short, Detail::Long, Detail::Long)
    );
    let page = Selected::root(&PROGRAM.root).help(Detail::Long).to_string();
    assert!(page.starts_with("a tool\n\nUsage: tool <command> [options]"), "{page}");
    assert!(page.contains("exec, x  run it"), "{page}");
}

#[test]
fn values_are_checked_and_words_after_the_end_of_options_are_positional() {
    assert_eq!(
        error("dump cst a --format=jsn"),
        (errors::CLI_INVALID_VALUE, "did you mean `json`?".to_owned())
    );
    let Ok(Request::Run(selected, args)) = PROGRAM.parse(&argv("x a.vun b -- --fast -c")) else {
        panic!("not a run")
    };
    assert_eq!(names(&selected), ["tool", "exec"]);
    assert_eq!(args.values(&REST).collect::<Vec<_>>(), ["b", "--fast", "-c"]);
}

#[test]
fn completions_cover_every_command_under_its_aliases() {
    let paths: Vec<_> = PROGRAM.tree().nodes.into_iter().map(|n| n.path).collect();
    assert_eq!(paths, [vec![], vec!["dump"], vec!["dump", "cst"], vec!["exec"], vec!["x"]]);
    for dialect in Dialect::ALL {
        assert!(!PROGRAM.completion(dialect).is_empty());
    }
}

#[test]
fn a_flag_takes_no_value() {
    assert_eq!(error("dump cst a.vun --fast=yes").0, errors::CLI_UNEXPECTED_ARGUMENT);
}

#[test]
fn usage_errors_suggest_and_point_at_help() {
    assert_eq!(error("dmp"), (errors::CLI_UNKNOWN_COMMAND, "did you mean `dump`?".to_owned()));
    assert_eq!(error("dump cst --fats a"), (errors::CLI_UNKNOWN_OPTION, "did you mean `--fast`?".to_owned()));
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
    assert!(matches!(PROGRAM.parse(&argv("-help")), Ok(Request::Help(..))));
}

#[test]
fn group_options_apply_below() {
    for line in ["--deep dump cst a.vun", "dump --deep cst a.vun", "dump cst a.vun --deep"] {
        let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv(line)) else { panic!("{line:?} did not run") };
        assert!(args.flag(&DEEP), "{line:?}");
    }
}

#[test]
fn globals_are_read_ahead_of_parsing_up_to_the_end_of_options() {
    let args = PROGRAM.globals(&argv("dmp --color never x -- --format json"));
    assert_eq!((args.value(&crate::COLOR), args.value(&crate::FORMAT)), (Some("never"), None));
    assert_eq!(PROGRAM.globals(&argv("--color=nevr")).value(&crate::COLOR), None);
}
