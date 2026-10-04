use std::path::PathBuf;

use vun_fs::Scratch;

use super::*;

#[test]
fn the_clone_holds_what_the_grammar_publishes_and_nothing_it_dropped() {
    let (grammar, clone) = (Scratch::new("publish-grammar"), Scratch::new("publish-clone"));
    for name in ["grammar.js", "conflicts.json", "tree-sitter.json", "src/parser.c", "queries/highlights.scm"]
    {
        grammar.write(name, name);
    }
    grammar.write("release.json", "not published");
    clone.write("queries/folds.scm", "dropped");
    clone.write("README.md", "the clone's own");
    let repository =
        Repository { dir: clone.path().to_owned(), jj: Executable::new("jj", PathBuf::from("jj")) };
    repository.mirror(grammar.path()).unwrap();
    let held = clone.path().files().unwrap();
    let held: Vec<_> = held.iter().map(|f| f.strip_prefix(clone.path()).unwrap().to_owned()).collect();
    let expected = [
        "README.md",
        "conflicts.json",
        "grammar.js",
        "queries/highlights.scm",
        "src/parser.c",
        "tree-sitter.json",
    ];
    assert_eq!(held, expected.map(PathBuf::from));
}
