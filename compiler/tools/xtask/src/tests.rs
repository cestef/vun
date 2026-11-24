use vun_harness::repo_root;

use std::path::{Path, PathBuf};

use vun_fs::Scratch;

use crate::corpus::Corpus;
use crate::decisions::{Decisions, Malformed, ReadError};
use crate::layers::{Breach, Placed, Ranks};
use crate::lines::{self, CHECKED, Oversized};
use crate::names::{Banned, Unnamed};

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
fn the_built_binary_is_the_last_executable_cargo_reports() {
    let messages = [
        r#"{"reason":"compiler-artifact","executable":null}"#,
        "not json",
        r#"{"reason":"compiler-artifact","executable":"/t/dist/vun"}"#,
        r#"{"reason":"build-finished","success":true}"#,
    ]
    .join("\n");
    assert_eq!(crate::cargo::executable(&messages), Some("/t/dist/vun".into()));
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

fn sources(name: &str) -> Scratch {
    let root = Scratch::new(name);
    for checked in CHECKED {
        root.write(format!("{}/.keep", checked.dir), "");
    }
    root
}

#[test]
fn a_file_past_the_limit_is_oversized() {
    let root = sources("oversized");
    root.write("compiler/a/long.rs", "//\n".repeat(lines::MAX_LINES as usize + 1));
    root.write("compiler/a/exact.rs", "//\n".repeat(lines::MAX_LINES as usize));
    let found = Oversized::all(root.path()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, PathBuf::from("compiler/a/long.rs"));
    assert_eq!(found[0].lines, lines::MAX_LINES + 1);
}

#[test]
fn every_banned_name_is_found_as_a_file_and_as_a_directory() {
    let root = sources("names");
    for banned in Banned::ALL {
        root.write(format!("compiler/g/c/src/{}.rs", banned.name()), "");
    }
    root.write("lib/pkg/common/ok.vun", "");
    root.write("lib/pkg/fine.vun", "");
    root.write("corpus/accept/types/ok.vun", "");
    let found = Unnamed::all(root.path()).unwrap();
    assert_eq!(found.len(), Banned::ALL.len() + 1);
    for banned in Banned::ALL {
        let path = PathBuf::from(format!("compiler/g/c/src/{}.rs", banned.name()));
        assert!(found.contains(&Unnamed { path, banned: *banned }));
    }
    assert!(found.contains(&Unnamed { path: "lib/pkg/common".into(), banned: Banned::Common }));
}

#[test]
fn the_sources_name_their_concerns() {
    let found = Unnamed::all(&repo_root()).expect("readable sources");
    assert!(found.is_empty(), "{found:?}; rename them for what they hold");
}

#[test]
fn unreadable_decision_trees_are_refused_with_their_reason() {
    let read = |files: &[(&str, &str)]| {
        let root = Scratch::new("decisions");
        for (name, text) in files {
            root.write(format!("{}/{name}", crate::decisions::DIR), text);
        }
        Decisions::read(root.path()).err().expect("a refusal")
    };
    let one = "# 1. A\n\nStatus: accepted\n";
    assert!(matches!(read(&[("notes.md", one)]), ReadError::Stray(_)));
    assert!(matches!(read(&[("002.md", one)]), ReadError::Misnumbered { held: 1, .. }));
    assert!(matches!(
        read(&[("001.md", "# 1. A\n\nStatus: accepted\nAmended by: 7\n")]),
        ReadError::Dangling { number: 1, cited: 7 }
    ));
    assert!(matches!(
        read(&[("001.md", "# 1. A\n")]),
        ReadError::Malformed { reason: Malformed::NoStatus, .. }
    ));
}
