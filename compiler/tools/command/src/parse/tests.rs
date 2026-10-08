use super::*;
use crate::{Catalog, Dialect, OptionSet, Outcome, PathKind, Shell, Values};
use std::io::Write;

const FILE: Param = Param::positional("file").about("a source file").values(Values::Path(PathKind::File));
const FORMAT: Param = Param::option("format")
    .short('o')
    .about("output format")
    .values(Values::Choice(|| vec!["json", "short"]));
const FAST: Param = Param::flag("fast").short('f').about("go fast");
const DEEP: Param = Param::flag("deep").about("look deeper");
const REST: Param = Param::positional("rest").arity(Arity::Many);
const MAYBE: Param = Param::positional("maybe").arity(Arity::Optional);
const KEEP: Param = Param::option("keep").values(Values::Catalog(Catalog {
    names: || vec!["names.duplicate", "names.unresolved"],
    what: "an error",
    declared_in: "errors.vun",
}));
const RARE: Param = Param::flag("rare").about("seldom wanted");
const TUNING: OptionSet = OptionSet { title: "Tuning options", params: &[RARE], detail: Detail::Long };
const TIMES: Param = Param::option("times").about("how many times").values(Values::Count);
/// Named as the root's `--version` flag, but an option of its command.
const RELEASE: Param = Param::option("version").about("which release");

fn ok(_: &mut (), _: &Args, shell: &mut Shell<'_>) -> std::io::Result<Outcome> {
    shell.out.flush().map(|()| Outcome::Success)
}

const PROGRAM: Program<()> = Program {
    defaults: crate::undefaulted,
    configure: crate::unconfigured,
    version: "1.0",
    build: None,
    root: Command::group(
        "tool",
        "a tool",
        &[
            Command::group(
                "dump",
                "print things",
                &[Command::run("cst", "the tree", ok).params(&[FILE, FORMAT, FAST])],
            ),
            Command::run("go", "run it too", ok).params(&[MAYBE, REST]).sets(&[TUNING]),
            Command::run("exec", "run it", ok).params(&[FILE, RELEASE, KEEP, TIMES, REST]).aliases(&["x"]),
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
    assert_eq!(paths, [vec![], vec!["dump"], vec!["dump", "cst"], vec!["go"], vec!["exec"], vec!["x"]]);
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

#[test]
fn a_catalog_is_checked_as_a_choice_and_described_rather_than_listed() {
    assert_eq!(error("x a.vun --keep names.unresolvd").1, "did you mean `names.unresolved`?");
    assert_eq!(error("x a.vun --keep zzz").1, "choose an error, as `errors.vun` declares them");
    let Ok(Request::Run(selected, _)) = PROGRAM.parse(&argv("x a.vun")) else { panic!("not a run") };
    let page = selected.help(Detail::Long).to_string();
    assert!(page.contains("[values: an error, as `errors.vun` declares them]"), "{page}");
    assert!(!page.contains("names.duplicate"), "{page}");
}

#[test]
fn a_count_is_checked_as_a_whole_number() {
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("x a.vun --times 3")) else { panic!("not a run") };
    assert_eq!(args.count(&TIMES), Some(3));
    assert_eq!(
        error("x a.vun --times three"),
        (errors::CLI_INVALID_VALUE, "write a whole number, such as `100`".to_owned())
    );
}

#[test]
fn words_after_the_end_of_options_skip_an_optional_positional() {
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("go -- a.bf")) else { panic!("not a run") };
    assert_eq!((args.value(&MAYBE), args.values(&REST).collect::<Vec<_>>()), (None, ["a.bf"].to_vec()));
    let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv("go . -- a.bf")) else { panic!("not a run") };
    assert_eq!((args.value(&MAYBE), args.values(&REST).collect::<Vec<_>>()), (Some("."), ["a.bf"].to_vec()));
}

#[test]
fn settings_fill_the_options_the_command_line_leaves_out() {
    use crate::{Setting, Settings};
    let setting = |option: &str, values: &[&str]| Setting {
        command: "dump".into(),
        option: option.into(),
        values: values.iter().map(|v| (*v).to_owned()).collect(),
    };
    // `dump` is a group; settings are by the last command selected, here `cst`.
    let settings = Settings::new(vec![
        Setting { command: "cst".into(), option: "format".into(), values: vec!["short".into()] },
        Setting { command: "cst".into(), option: "fast".into(), values: vec!["true".into()] },
        setting("format", &["json"]),
    ]);
    let Ok(Request::Run(selected, mut args)) = PROGRAM.parse(&argv("dump cst a.vun")) else {
        panic!("not a run")
    };
    args.defaulted(&selected, &settings).unwrap();
    assert_eq!((args.value(&FORMAT), args.flag(&FAST)), (Some("short"), true));
    // A value on the command line wins.
    let Ok(Request::Run(selected, mut args)) = PROGRAM.parse(&argv("dump cst a.vun --format json")) else {
        panic!("not a run")
    };
    args.defaulted(&selected, &settings).unwrap();
    assert_eq!(args.value(&FORMAT), Some("json"));
    // A value that is not a choice is a usage error.
    let bad = Settings::new(vec![Setting {
        command: "cst".into(),
        option: "format".into(),
        values: vec!["xml".into()],
    }]);
    let Ok(Request::Run(selected, mut args)) = PROGRAM.parse(&argv("dump cst a.vun")) else {
        panic!("not a run")
    };
    assert_eq!(args.defaulted(&selected, &bad).unwrap_err().id, errors::CLI_INVALID_VALUE);
}

#[test]
fn a_flag_repeated_in_one_word_is_given_as_often() {
    for (line, times) in [("x a.vun", 0), ("x a.vun -v", 1), ("x a.vun -vv", 2), ("x -v a.vun -v -vv", 4)] {
        let Ok(Request::Run(_, args)) = PROGRAM.parse(&argv(line)) else { panic!("not a run: {line}") };
        assert_eq!(args.times(&crate::VERBOSE), times, "{line}");
    }
}

#[test]
fn a_set_of_rare_options_is_left_to_the_long_page_but_accepted_everywhere() {
    let Ok(Request::Run(path, args)) = PROGRAM.parse(&argv("go --rare")) else { panic!("not a run") };
    assert!(args.flag(&RARE));
    let short = path.help(Detail::Short).to_string();
    let long = path.help(Detail::Long).to_string();
    assert!(!short.contains("--rare") && !short.contains("Tuning options"), "{short}");
    assert!(long.contains("--rare") && long.contains("Tuning options"), "{long}");
}
