use vun_harness::repo_root;

use super::{Editors, Model, Restriction, Term};
use crate::generate::{Generator, Source};
use crate::grammar::{SYNTAX, UNRESOLVED};

/// The texts of the grammar's files, by path.
fn texts() -> Vec<(String, String)> {
    let dir = repo_root().join(SYNTAX);
    let mut texts: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the grammar's directory")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == vun_span::EXTENSION))
        .map(|p| {
            let path = p.strip_prefix(repo_root()).expect("inside the repository").display().to_string();
            (path, std::fs::read_to_string(&p).expect("readable"))
        })
        .collect();
    texts.sort();
    texts
}

fn sources(texts: &[(String, String)]) -> Vec<Source<'_>> {
    texts
        .iter()
        .map(|(path, text)| Generator::parse(path, text).unwrap_or_else(|d| panic!("{d:#?}")))
        .collect()
}

#[test]
fn the_grammar_resolves() {
    let texts = texts();
    let sources = sources(&texts);
    let model = Model::read(&sources).unwrap_or_else(|d| panic!("{d:#?}"));
    Editors::read(&model, &sources).unwrap_or_else(|d| panic!("{d:#?}"));
    assert_eq!(model.infix_kind, model.kind("infix"));
    let binding = model.kind("binding").expect("a binding kind");
    let fields: Vec<&str> = model.slots(binding).iter().map(|s| model.field_name(s.field)).collect();
    assert_eq!(fields, ["mut", "pattern", "value"]);
    // `.of(.type, grammar.literal_types)` names the table by its path, and lists its kinds.
    let typed_lit = model.production(model.kind("typed_lit").expect("typed_lit")).expect("a production");
    let Term::Field(_, Some(Restriction::Of(table))) = &typed_lit[0] else { panic!("an `.of` field") };
    assert_eq!(table.name, "literal_types");
    assert_eq!(
        table.members,
        [model.kind("name"), model.kind("member"), model.kind("call")].map(Option::unwrap)
    );
    assert!(!model.operand_contexts.is_empty());
}

#[test]
fn a_path_naming_nothing_is_unresolved() {
    let mut texts = texts();
    let nodes = texts.iter_mut().find(|(path, _)| path.ends_with("nodes.vun")).expect("nodes.vun");
    nodes.1 = nodes.1.replace("grammar.literal_types", "grammar.nowhere");
    let sources = sources(&texts);
    let Err(diagnostic) = Model::read(&sources) else { panic!("a path naming nothing resolved") };
    assert_eq!(diagnostic.id, UNRESOLVED);
}
