use vun_harness::repo_root;

use super::entry::{Decision, Malformed, Status};
use super::{DIR, Decisions};

const SAMPLE: &str = "# 12. Interfaces are functions\n\nStatus: accepted\nWhere: 02, 13\nAmended by: 14, 20\nNote: the user's choice\n\nThe text.\n";

#[test]
fn a_decision_file_gives_its_facts() {
    let decision = Decision::parse(SAMPLE).unwrap();
    assert_eq!(decision.number, 12);
    assert_eq!(decision.title, "Interfaces are functions");
    assert_eq!(decision.status, Status::Accepted);
    assert_eq!(decision.places, ["02", "13"]);
    assert_eq!(decision.amended_by, [14, 20]);
    assert_eq!(decision.note.as_deref(), Some("the user's choice"));
}

#[test]
fn a_malformed_file_says_what_is_wrong() {
    let parse = |text: &str| Decision::parse(text).unwrap_err();
    assert_eq!(parse("Interfaces\n"), Malformed::Title);
    assert_eq!(parse("# 1. \n\nStatus: accepted\n"), Malformed::Title);
    assert_eq!(parse("# 1. A\n\nWhere: 01\n\nText\n"), Malformed::NoStatus);
    assert_eq!(parse("# 1. A\n\nStatus: accepted\nStatus: proposed\n"), Malformed::Repeated("Status"));
    assert_eq!(parse("# 1. A\n\nStatus: maybe\n"), Malformed::Value { line: 3 });
    assert_eq!(parse("# 1. A\n\nMood: calm\n"), Malformed::UnknownFact { line: 3 });
    assert_eq!(parse("# 1. A\n\nStatus: superseded\n"), Malformed::Succession);
    assert_eq!(parse("# 1. A\n\nStatus: accepted\nSuperseded by: 2\n"), Malformed::Succession);
}

#[test]
fn every_decision_of_the_repository_reads() {
    let decisions = Decisions::read(&repo_root()).expect("decisions that read");
    assert!(decisions.all().len() > 500, "the decisions are all read");
}

#[test]
fn the_index_holds_what_the_decisions_give() {
    let root = repo_root();
    let held = std::fs::read_to_string(root.join(DIR).join("README.md")).expect("a written index");
    assert_eq!(held, Decisions::read(&root).unwrap().index(), "run `cargo xtask decisions`");
}
