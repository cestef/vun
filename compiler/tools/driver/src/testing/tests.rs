use super::*;

#[test]
fn a_selection_is_given_to_the_runner_as_names_then_the_filter() {
    let selection = Selection { names: vec!["a", "b"], filter: Some("c") };
    assert_eq!(selection.arguments(), ["a", "b", FILTER.long_form().as_str(), "c"]);
    assert!(!selection.is_empty());
    assert!(Selection::default().is_empty());
    assert!(Selection::default().arguments().is_empty());
}

#[test]
fn a_mode_chooses_among_its_own_kind_of_test() {
    let nouns: Vec<_> = [Mode::Test, Mode::Cover, Mode::Bench, Mode::Fuzz].map(|m| m.noun().one).into();
    assert_eq!(nouns, ["test", "test", "benchmark", "property"]);
}
