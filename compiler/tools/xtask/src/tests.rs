use vun_harness::repo_root;

use crate::corpus::{self, Corpus};
use crate::lines::{self, Oversized};

#[test]
fn corpus_has_no_failures() {
    let outcomes = Corpus { root: &repo_root() }.outcomes().expect("readable corpus");
    let failed: Vec<_> = outcomes
        .iter()
        .filter(|o| {
            matches!(
                o.verdict,
                corpus::Verdict::Fail(_) | corpus::Verdict::Unexplained(_) | corpus::Verdict::Ready
            )
        })
        .map(|o| &o.path)
        .collect();
    assert!(failed.is_empty(), "failing corpus files: {failed:?}; run `cargo xtask corpus`");
}

#[test]
fn no_file_is_monolithic() {
    let oversized = Oversized::all(&repo_root()).expect("readable sources");
    let listed: Vec<_> =
        oversized.iter().map(|o| format!("{} ({} lines)", o.path.display(), o.lines)).collect();
    assert!(listed.is_empty(), "files past {} lines: {listed:?}; split them by concern", lines::MAX_LINES);
}

#[test]
fn the_measured_binary_is_the_last_executable_cargo_reports() {
    let messages = [
        r#"{"reason":"compiler-artifact","executable":null}"#,
        "not json",
        r#"{"reason":"compiler-artifact","executable":"/t/dist/vun"}"#,
        r#"{"reason":"build-finished","success":true}"#,
    ]
    .join("\n");
    assert_eq!(crate::size::Measured::executable(&messages), Some("/t/dist/vun".into()));
}
