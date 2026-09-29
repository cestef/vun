use crate::check::Checked;
use crate::tests::packages;

/// The messages checking `app` reports, in source order.
fn reported(app: &str) -> Vec<String> {
    let (db, file) = packages(app);
    let reports = db.get::<Checked>(&file).unwrap();
    reports
        .sorted(vun_origin::OriginTable::global())
        .iter()
        .map(|d| format!("{}: {}", d.id, d.message))
        .collect()
}

#[test]
fn function_types_are_values() {
    let app = "\
f: (u8) -> u8 = 3;
g: (u8) -> u8 = (x: u8) -> Int(0..=9) { 0 };
h: (u8) -> u8 = (x: u8) -> u8 { x };
";
    assert_eq!(
        reported(app),
        [
            "types.mismatch: expected `(u8) -> u8`, found `Int(3..=3)`",
            "types.mismatch: expected `(u8) -> u8`, found `(Int(0..=255)) -> Int(0..=9)`",
        ]
    );
}

#[test]
fn nominal_types_are_distinct_and_named_by_their_declaration() {
    let app = "\
NodeId := struct { raw: u8 };
TypeId := struct { raw: u8 };
type_of := (n: NodeId) -> TypeId { n };
same := (n: NodeId) -> NodeId { n };
";
    assert_eq!(reported(app), ["types.mismatch: expected `TypeId`, found `NodeId`"]);
}
