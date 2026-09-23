use crate::Primitive;
use crate::primitives::{lang, mem};

#[test]
fn primitives_are_the_top_level_holes_of_lang_by_namespace() {
    assert!(Primitive::ALL.contains(&Primitive::Mem(mem::Mem::Ref)));
    assert_eq!((Primitive::Lang(lang::Lang::Int).namespace(), lang::Lang::Int.name()), ("lang", "Int"));
    assert_eq!(Primitive::Lang(lang::Lang::r#type).name(), "type");
    // Requirements of interfaces are not primitives: `lang.ops.Add.add` is a hole inside a call.
    assert!(Primitive::ALL.iter().all(|p| p.namespace() != "lang.ops"), "{:?}", Primitive::ALL);
}

/// A database holding a `lang` with an integer primitive and an app seeing it through its
/// prelude, the app's root file holding `app`.
fn packages(app: &str) -> (overt_query::Db, overt_hir::db::FileRef) {
    use std::sync::Arc;

    use overt_hash::Hash;
    use overt_hir::db::{DepData, FileRef, FileText, Package, PackageData, PackageId};
    use overt_intern::Symbol;

    let db = overt_query::Db::new();
    let (lang, user) = (PackageId(Hash::of(b"lang")), PackageId(Hash::of(b"app")));
    let text = "Int: ($r: Range) -> type = _;\nRange: type = _;\nbool := enum { false, true };\nu8 := Int(0..=255);\n";
    let files = |name: &str| vec![format!("{name}.ov")];
    let data = |name: &str, deps, prelude| PackageData {
        name: Some(Symbol::intern(name)),
        dir: format!("/{name}"),
        key: format!("{name}@1.0.0"),
        deps,
        prelude,
        files: files(name),
        ..PackageData::default()
    };
    db.set::<Package>(lang, Arc::new(data("lang", Vec::new(), Vec::new())));
    let dep = DepData { name: Symbol::intern("lang"), package: Ok(lang) };
    db.set::<Package>(user, Arc::new(data("app", vec![dep], vec![vec![Symbol::intern("lang")]])));
    db.set::<FileText>(FileRef { package: lang, path: "lang.ov".into() }, Some(text.into()));
    let file = FileRef { package: user, path: "app.ov".into() };
    db.set::<FileText>(file.clone(), Some(app.into()));
    (db, file)
}

#[test]
fn values_are_checked_against_their_types_through_the_prelude() {
    let (db, file) = packages("flag: bool = 3;\nsmall: u8 = 200;\nlarge: u8 = 300;\nexact: Int(1..3) = 2;\n");
    let reports = db.get::<crate::check::Checked>(&file).unwrap();
    let messages: Vec<String> = reports.sorted().iter().map(|d| d.message.to_string()).collect();
    assert_eq!(
        messages,
        ["expected `bool`, found `Int(3..=3)`", "expected `u8`, found `Int(300..=300)`"],
        "{reports:?}"
    );
}
