use super::*;
use overt_fs::Scratch;
use std::collections::HashSet;

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
    let path = dir.write("dump.ov", "x := 1;\n");
    let argv = ["dump", "cst", path.to_str().unwrap()].map(str::to_owned);
    let (mut session, mut out) = (Session::new(), Vec::new());
    let outcome = overt_command::run(&PROGRAM, &argv, &mut session, &mut out).unwrap();
    assert_eq!(outcome, Outcome::Success);
    assert!(String::from_utf8(out).unwrap().starts_with("File Eof 8..8"));
}
