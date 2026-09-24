use vun_harness::repo_root;

use crate::corpus::{self, Corpus};
use crate::generate;
use crate::lines::{self, Oversized};

#[test]
fn corpus_has_no_failures() {
    let outcomes = Corpus { root: &repo_root() }.outcomes().expect("readable corpus");
    let failed: Vec<_> = outcomes
        .iter()
        .filter(|o| matches!(o.verdict, corpus::Verdict::Fail(_) | corpus::Verdict::Unexplained(_)))
        .map(|o| &o.path)
        .collect();
    assert!(failed.is_empty(), "failing corpus files: {failed:?}; run `cargo xtask corpus`");
}

#[test]
fn generated_sources_are_current() {
    let root = repo_root();
    for generator in generate::ALL {
        let generated = generator.generate(&root).unwrap_or_else(|d| panic!("{d:#?}"));
        let changes = generator.changes(&root, &generated).expect("readable output");
        assert!(changes.is_empty(), "{} is stale; run `cargo xtask generate`", generator.output);
    }
}

#[test]
fn no_file_is_monolithic() {
    let oversized = Oversized::all(&repo_root()).expect("readable sources");
    let listed: Vec<_> =
        oversized.iter().map(|o| format!("{} ({} lines)", o.path.display(), o.lines)).collect();
    assert!(listed.is_empty(), "files past {} lines: {listed:?}; split them by concern", lines::MAX_LINES);
}

#[test]
fn a_file_no_longer_generated_is_stale() {
    let root = vun_fs::Scratch::new("xtask-stale");
    let output = root.join(generate::ALL[0].output);
    root.write(output.join("mod.rs"), "current");
    root.write(output.join("removed.rs"), "old");
    let generated = std::collections::BTreeMap::from([("mod.rs".to_owned(), "current".to_owned())]);
    let changes = generate::ALL[0].changes(&root.join(""), &generated).expect("readable scratch");
    assert!(changes.differing.is_empty());
    assert_eq!(changes.stale, [output.join("removed.rs")]);
}
