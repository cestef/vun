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

/// `vun debug dump <view> <file> --at <place>`: what it wrote, or its outcome.
fn dumped_at(view: &str, path: &std::path::Path, at: &str) -> Result<String, Outcome> {
    let argv = ["debug", "dump", view, path.to_str().unwrap(), "--at", at].map(str::to_owned);
    let (mut session, mut out, mut err) = (Session::new(), Vec::new(), Vec::new());
    let outcome = PROGRAM.run(&argv, &mut session, &mut Shell::plain(&mut out, &mut err));
    match outcome.map_err(|refusal| refusal.outcome)? {
        Outcome::Success => Ok(String::from_utf8(out).unwrap()),
        failed => Err(failed),
    }
}

#[test]
fn a_dump_at_a_place_shows_the_unit_holding_it() {
    let dir = Scratch::new("dump-at");
    let path = dir.write("at.vun", "f := (x: u8) -> u8 { x };\ng := (y: u16) -> u16 { y };\n");
    let types = dumped_at("types", &path, "2:24").unwrap();
    assert_eq!(types.matches("fn ").count(), 1, "{types}");
    let values = dumped_at("values", &path, "1:1").unwrap();
    assert_eq!(values.lines().count(), 1, "{values}");
    assert!(values.contains(" f"), "{values}");
    assert_eq!(dumped_at("types", &path, "9:1"), Err(Outcome::Failure), "a line the file lacks");
    assert_eq!(dumped_at("types", &path, "nowhere"), Err(Outcome::Usage), "no place");
}
