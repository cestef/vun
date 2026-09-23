use overt_harness::repo_root;

use crate::{corpus, generate, lines};

#[test]
fn corpus_has_no_failures() {
    let outcomes = corpus::run(&repo_root()).expect("readable corpus");
    let failed: Vec<_> =
        outcomes.iter().filter(|o| matches!(o.verdict, corpus::Verdict::Fail(_))).map(|o| &o.path).collect();
    assert!(failed.is_empty(), "failing corpus files: {failed:?}; run `cargo xtask corpus`");
}

#[test]
fn generated_sources_are_current() {
    let root = repo_root();
    for generator in generate::ALL {
        let generated = generator.generate(&root).unwrap_or_else(|d| panic!("{d:#?}"));
        let current = std::fs::read_to_string(root.join(generator.output)).expect("generated file exists");
        assert!(current == generated, "{} is stale; run `cargo xtask generate`", generator.output);
    }
}

#[test]
fn no_file_is_monolithic() {
    let oversized = lines::oversized(&repo_root()).expect("readable sources");
    let listed: Vec<_> =
        oversized.iter().map(|o| format!("{} ({} lines)", o.path.display(), o.lines)).collect();
    assert!(listed.is_empty(), "files past {} lines: {listed:?}; split them by concern", lines::MAX_LINES);
}
