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

/// What `vun test <words>` takes as its path, and as the names of its tests.
fn chosen(words: &[&str]) -> (Option<std::path::PathBuf>, Vec<String>) {
    let line: Vec<String> = ["test"].into_iter().chain(words.iter().copied()).map(str::to_owned).collect();
    let Ok(vun_command::Request::Run(_, args)) = PROGRAM.parse(&line) else {
        panic!("{words:?} did not parse")
    };
    let chosen = test::Chosen::of(&args);
    (chosen.path.map(Path::to_path_buf), chosen.names.into_iter().map(str::to_owned).collect())
}

#[test]
fn a_first_word_naming_nothing_on_disk_is_a_test() {
    let dir = Scratch::new("test-words");
    let path = dir.write("main.vun", "");
    let file = path.to_str().unwrap();
    assert_eq!(chosen(&[]), (None, vec![]));
    assert_eq!(chosen(&["crc_vector"]), (None, vec!["crc_vector".to_owned()]));
    assert_eq!(chosen(&[file, "a", "b"]), (Some(path.clone()), vec!["a".to_owned(), "b".to_owned()]));
    assert_eq!(chosen(&["missing.vun", "a"]), (None, vec!["missing.vun".to_owned(), "a".to_owned()]));
}

/// `vun build <dir> [--lint ...]` on a package whose only file declares a function no program
/// reaches, named against `std.lints.naming.case`, its outcome.
fn built_with(dir: &Scratch, lints: &[&str]) -> Outcome {
    let argv: Vec<String> = ["build".to_owned(), dir.path().display().to_string()]
        .into_iter()
        .chain(lints.iter().flat_map(|level| ["--lint".to_owned(), (*level).to_owned()]))
        .collect();
    let (mut session, mut out, mut err) = (Session::new(), Vec::new(), Vec::new());
    PROGRAM.run(&argv, &mut session, &mut Shell::plain(&mut out, &mut err)).unwrap()
}

#[test]
fn a_build_of_a_package_checks_the_files_no_program_reaches() {
    let dir = Scratch::new("build-checks-files");
    let manifest = "\
package := .{
    .name = \"lone\",
    .id = 0x0000_0000_0000_0f02,
    .version = \"0.0.0\",
    .edition = 2026,
    .include = .{ \"build.vun\", \"lone.vun\" },
    .deps = .{},
};

build := (b: &mut std.build.Build) {};
";
    dir.write("build.vun", manifest);
    dir.write("lone.vun", "WriteAll := () -> u8 { 0 };\n");
    assert_eq!(built_with(&dir, &[]), Outcome::Failure);
    assert_eq!(built_with(&dir, &["std.lints.naming.case=warn"]), Outcome::Success);
}
