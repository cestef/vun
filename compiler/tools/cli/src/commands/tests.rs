use super::*;
use std::collections::HashSet;
use vun_fs::Scratch;

fn check_names(command: &Command<Session>) {
    let children = command.children();
    let names: HashSet<_> = children.iter().map(|c| c.name).collect();
    assert_eq!(names.len(), children.len(), "duplicate commands under {}", command.name);
    let params: HashSet<_> = command.params.iter().map(|p| p.name).collect();
    assert_eq!(params.len(), command.params.len(), "duplicate parameters of {}", command.name);
    children.iter().for_each(check_names);
}

#[test]
fn names_are_unique_at_every_level() {
    check_names(&PROGRAM.root);
}

#[test]
fn dump_cst_prints_the_tree() {
    let dir = Scratch::new("dump");
    let path = dir.write("dump.vun", "x := 1;\n");
    let argv = ["debug", "dump", "cst", path.to_str().unwrap()].map(str::to_owned);
    let (mut session, mut out, mut err) = (Session::new(), Vec::new(), Vec::new());
    let outcome = PROGRAM.run(&argv, &mut session, &mut Shell::plain(&mut out, &mut err)).unwrap();
    assert_eq!(outcome, Outcome::Success);
    assert!(String::from_utf8(out).unwrap().starts_with("File Eof 8..8"));
}

/// `vun check <file> --lint ...`, its outcome.
fn checked(path: &std::path::Path, lint: &str) -> Outcome {
    let argv = ["check", path.to_str().unwrap(), "--lint", lint].map(str::to_owned);
    let (mut session, mut out, mut err) = (Session::new(), Vec::new(), Vec::new());
    PROGRAM.run(&argv, &mut session, &mut Shell::plain(&mut out, &mut err)).unwrap()
}

#[test]
fn the_command_line_sets_a_lint_level() {
    let dir = Scratch::new("lint-level");
    let path = dir.write("app.vun", "WriteAll := () -> u8 { 0 };\n");
    // `naming.case` is `deny` by default, so lowering it to `warn` lets the file pass.
    assert_eq!(checked(&path, "std.lints.naming.case=warn"), Outcome::Success);
    // A name matching no lint or group, and a level that is none of `lang.lint.Level`'s.
    assert_eq!(checked(&path, "std.lints.naming.kase=warn"), Outcome::Failure);
    assert_eq!(checked(&path, "std.lints.naming.case=war"), Outcome::Failure);
}
