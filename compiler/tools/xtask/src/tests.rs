use vun_harness::repo_root;

use std::path::Path;

use crate::corpus::Corpus;
use crate::layers::{Breach, Placed, Ranks};
use crate::lines::{self, Oversized};

#[test]
fn corpus_has_no_failures() {
    let outcomes = Corpus::at(&repo_root()).outcomes().expect("readable corpus");
    let failed: Vec<_> = outcomes.iter().filter(|o| o.verdict.is_failure()).map(|o| &o.path).collect();
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

#[test]
fn the_layers_diagram_reads_the_ranks() {
    let held = std::fs::read_to_string(repo_root().join(Ranks::TABLE)).expect("readable table");
    assert_eq!(held, Ranks::toml(), "run `cargo xtask layers`");
}

#[test]
fn no_crate_depends_on_a_later_group() {
    let breaches = Breach::all(&repo_root()).expect("cargo metadata");
    assert!(breaches.is_empty(), "{breaches:?}; run `cargo xtask layers`");
}

#[test]
fn a_dependency_on_a_peer_or_later_group_and_an_unranked_group_are_breaches() {
    let package = |name: &str, group: &str, deps: &[&str]| {
        let deps: Vec<_> = deps.iter().map(|d| format!(r#"{{"name":"{d}"}}"#)).collect();
        format!(
            r#"{{"name":"{name}","manifest_path":"/r/compiler/{group}/{name}/Cargo.toml","dependencies":[{}]}}"#,
            deps.join(",")
        )
    };
    let packages = [
        package("a", "base", &[]),
        package("b", "front", &["a", "c"]),
        package("c", "tools", &["b"]),
        package("d", "back", &["e"]),
        package("e", "link", &[]),
        package("f", "misc", &["a"]),
    ];
    let metadata = format!(r#"{{"packages":[{}]}}"#, packages.join(","));
    let placed = |name: &str, group: &str| Placed { name: name.to_owned(), group: group.to_owned() };
    assert_eq!(
        Breach::within(&metadata, Path::new("/r/compiler")),
        [
            Breach::Unranked(placed("f", "misc")),
            Breach::Upward { from: placed("b", "front"), to: placed("c", "tools") },
            Breach::Upward { from: placed("d", "back"), to: placed("e", "link") },
        ]
    );
}

#[test]
fn only_the_tasks_declaring_words_pass_them_on() {
    use crate::task;
    assert!(task::PUSH.passes_words());
    assert!(task::CHECK.needs.iter().all(|t| !t.passes_words()));
}
