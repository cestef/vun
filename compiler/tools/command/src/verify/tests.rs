use super::*;
use crate::{Args, CommandVerb, Outcome, Shell};
use std::io::Write;

fn nothing(_: &mut (), _: &Args, shell: &mut Shell<'_>) -> std::io::Result<Outcome> {
    shell.out.flush().map(|()| Outcome::Success)
}

const SHOW: Command<()> = Command::run(CommandVerb::Show, "show", nothing);
const LIST: Command<()> = Command::run(CommandVerb::List, "list", nothing);
const STRAY: Command<()> = Command::item("stray", "named by no verb", nothing);

#[test]
fn a_tree_of_verbs_passes() {
    const TREE: Command<()> =
        Command::group("tool", "tool", &[SHOW, Command::group("store", "store", &[LIST])]);
    assert_eq!(TREE.verify(), []);
}

#[test]
fn items_pass_below_a_verb_group() {
    const TREE: Command<()> =
        Command::group("tool", "tool", &[Command::verb_group(CommandVerb::Dump, "dump", &[STRAY])]);
    assert_eq!(TREE.verify(), []);
}

#[test]
fn a_leaf_outside_the_table_is_reported() {
    const TREE: Command<()> = Command::group("tool", "tool", &[SHOW, STRAY]);
    assert_eq!(TREE.verify(), [Violation::NotAVerb { path: vec!["tool", "stray"] }]);
}

#[test]
fn a_name_two_siblings_answer_to_is_reported() {
    const TREE: Command<()> = Command::group("tool", "tool", &[SHOW, LIST.aliases(&["show"])]);
    assert_eq!(TREE.verify(), [Violation::Ambiguous { path: vec!["tool"], name: "show" }]);
}

#[test]
fn every_verb_has_its_own_name() {
    let names: std::collections::HashSet<_> = CommandVerb::ALL.iter().map(|v| v.name()).collect();
    assert_eq!(names.len(), CommandVerb::ALL.len());
}

#[test]
fn the_legacy_leaves_are_the_six_nouns() {
    let legacy: Vec<_> = CommandVerb::legacy().collect();
    assert_eq!(
        legacy,
        [
            CommandVerb::Coverage,
            CommandVerb::Queries,
            CommandVerb::Profile,
            CommandVerb::Smith,
            CommandVerb::Completion,
            CommandVerb::Lsp,
        ]
    );
}
