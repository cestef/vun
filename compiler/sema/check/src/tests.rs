use crate::Checked;

/// A database holding a `lang` with an integer primitive and an app seeing it through its
/// prelude, the app's root file holding `app`.
fn packages(app: &str) -> (vun_query::Db, vun_hir::db::FileRef) {
    use std::sync::Arc;

    use vun_hash::Hash;
    use vun_hir::db::{DepData, FileRef, FileText, Package, PackageData, PackageId};
    use vun_intern::Symbol;

    let db = vun_query::Db::new();
    let (lang, user) = (PackageId::Stored(Hash::of(b"lang")), PackageId::Stored(Hash::of(b"app")));
    let text = "type: type = _;\nInt: ($r: Range) -> type = _;\nRange: type = _;\nbool := enum { false, true };\nu8 := Int(0..=255);\n";
    let files = |name: &str| vec![format!("{name}.vun")];
    let data = |name: &str, deps, prelude| PackageData {
        name: Some(Symbol::intern(name)),
        dir: format!("/{name}"),
        deps,
        prelude,
        files: files(name),
        ..PackageData::default()
    };
    db.set::<Package>(&lang, Arc::new(data("lang", Vec::new(), Vec::new())));
    let dep = DepData { name: Symbol::intern("lang"), package: Ok(lang) };
    db.set::<Package>(&user, Arc::new(data("app", vec![dep], vec![vec![Symbol::intern("lang")]])));
    db.set::<FileText>(&FileRef { package: lang, path: Symbol::intern("lang.vun") }, Some(text.into()));
    let file = FileRef { package: user, path: Symbol::intern("app.vun") };
    db.set::<FileText>(&file, Some(app.into()));
    (db, file)
}

#[test]
fn values_are_checked_against_their_types_through_the_prelude() {
    let (db, file) = packages("flag: bool = 3;\nsmall: u8 = 200;\nlarge: u8 = 300;\nexact: Int(1..3) = 2;\n");
    let reports = db.get::<Checked>(&file).unwrap();
    let messages: Vec<String> =
        reports.sorted(vun_origin::OriginTable::global()).iter().map(|d| d.message.to_string()).collect();
    assert_eq!(
        messages,
        ["expected `bool`, found `Int(3..=3)`", "value does not fit the type `u8`"],
        "{reports:?}"
    );
}

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
            "types.mismatch: expected `(u8) -> u8`, found `(u8) -> Int(0..=9)`",
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

#[test]
fn an_annotation_is_a_type() {
    let app = "\
Pair := ($T: type) -> type { struct { a: T, b: T } };
x := Pair { .a = 1, .b = 2 };
y := Pair(u8){ .a = 1, .b = 2 };
";
    assert_eq!(reported(app), ["types.mismatch: expected a type, found a value"]);
}
