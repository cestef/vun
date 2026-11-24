//! What hovering shows (spec 13, section 6): the name, its type and stage, and its layout.

use vun_sema::typed::Stage;
use vun_sema_ty::{TyKind, Value};

use crate::support::Fixture;

const SOURCE: &str = "limit: u64 = 3;\nf := (x: u8) -> u8 { x };\n";

/// The column of the first `needle` in the second line, hovered, as its offset.
fn offset_of(needle: &str, line: usize) -> u32 {
    let before: usize = SOURCE.lines().take(line).map(|l| l.len() + 1).sum();
    u32::try_from(before + SOURCE.lines().nth(line).unwrap().find(needle).unwrap()).unwrap()
}

#[test]
fn a_declaration_name_and_a_type_annotation_show_their_type_and_layout() {
    let fx = Fixture::new("hover-annotation");
    let path = fx.root.write("main.vun", SOURCE);
    let mut session = fx.session();
    let file = session.load(&path).unwrap();

    let name = session.hovered(file, offset_of("limit", 0)).expect("a declaration's own name");
    assert_eq!(name.known.map(|k| k.stage), Some(Stage::Compile));

    let annotation = session.hovered(file, offset_of("u64", 0)).expect("a type annotation");
    let known = annotation.known.expect("a type written as an annotation has a type");
    assert_eq!((known.ty, known.stage), (TyKind::of_types(), Stage::Compile));
    assert!(matches!(annotation.value, Some(Value::Type(_))));
    assert!(annotation.laid.is_some());

    let parameter = session.hovered(file, offset_of("u8", 1)).expect("a parameter's annotation");
    assert!(parameter.known.is_some());
}
