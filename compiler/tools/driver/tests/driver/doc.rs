//! API documentation through the driver: the reader's model, with the declarations of types told
//! from values by their types.

use vun_doc::{Docs, Kind};

use crate::common::Fixture;

#[test]
fn a_declaration_whose_type_is_type_is_documented_as_a_type() {
    let fixture = Fixture::new("doc-kinds");
    let manifest = Fixture::manifest("pkg", 1, "0.1.0");
    let dir = fixture.write(
        "pkg",
        &manifest,
        "\
/// A word.
Word := u32;
/// A count.
count := 3;
/// Doubles.
double := (x: u8) -> u16 { x * 2 };
",
    );
    let mut session = fixture.session();
    let mut docs = Docs::default();
    session.document(&mut docs, &dir).unwrap();
    let kinds: Vec<(&str, Kind)> =
        docs.namespaces[&vec!["pkg".to_owned()]].items.iter().map(|i| (i.name.as_str(), i.kind)).collect();
    assert_eq!(kinds, [("Word", Kind::Type), ("count", Kind::Value), ("double", Kind::Function)]);
}
