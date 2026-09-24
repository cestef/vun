use crate::Primitive;
use crate::primitives::{lang, mem};

#[test]
fn primitives_are_the_top_level_holes_of_lang_by_namespace() {
    assert!(Primitive::ALL.contains(&Primitive::Mem(mem::Mem::Ref)));
    assert_eq!(Primitive::Lang(lang::Lang::Int).item(), vun_items::lang::Int);
    assert_eq!(Primitive::Lang(lang::Lang::Int).namespace(), vun_items::lang);
    assert_eq!(Primitive::Lang(lang::Lang::r#type).name(), "type");
    // Requirements of interfaces are not primitives: `lang.ops.Add.add` is a hole inside a call.
    assert!(Primitive::ALL.iter().all(|p| p.namespace() != vun_items::lang::ops), "{:?}", Primitive::ALL);
}

#[test]
fn members_of_types_have_handles_by_their_path() {
    assert_eq!(vun_items::std::build_Build::new.path(), ["std", "build", "Build", "new"]);
    assert_eq!(vun_items::std::build_Action::dep.to_string(), "std.build.Action.dep");
    assert_eq!(vun_items::lang::target_Endian::little.name(), "little");
}

/// A database holding a `lang` with an integer primitive and an app seeing it through its
/// prelude, the app's root file holding `app`.
pub(crate) fn packages(app: &str) -> (vun_query::Db, vun_hir::db::FileRef) {
    use std::sync::Arc;

    use vun_hash::Hash;
    use vun_hir::db::{DepData, FileRef, FileText, Package, PackageData, PackageId};
    use vun_intern::Symbol;

    let db = vun_query::Db::new();
    let (lang, user) = (PackageId(Hash::of(b"lang")), PackageId(Hash::of(b"app")));
    let text = "type: type = _;\nInt: ($r: Range) -> type = _;\nRange: type = _;\nbool := enum { false, true };\nu8 := Int(0..=255);\n";
    let files = |name: &str| vec![format!("{name}.vun")];
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
    db.set::<FileText>(FileRef { package: lang, path: "lang.vun".into() }, Some(text.into()));
    let file = FileRef { package: user, path: "app.vun".into() };
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
        ["expected `bool`, found `Int(3..=3)`", "value does not fit the type `u8`"],
        "{reports:?}"
    );
}

#[test]
fn narrowing_traps_wraps_and_saturates() {
    use vun_desugar::Narrowing;

    use crate::env::Unevaluated;
    use crate::int::{Integer, Interval};
    use crate::trap::Reason;
    use crate::ty::Value;

    let u8 = Interval::closed(Integer::from(0), Integer::from(255)).unwrap();
    let int = |n: i32| Value::Int(Integer::from(n));
    let narrow = |n, how| int(n).narrow(&u8, how);
    assert_eq!(narrow(200, Narrowing::To), Ok(int(200)));
    assert_eq!(narrow(260, Narrowing::To), Err(Unevaluated::trap(Reason::Narrowing)));
    assert_eq!(narrow(260, Narrowing::Wrap), Ok(int(4)));
    assert_eq!(narrow(-1, Narrowing::Wrap), Ok(int(255)));
    assert_eq!(narrow(-7, Narrowing::Sat), Ok(int(0)));
    assert_eq!(narrow(999, Narrowing::Sat), Ok(int(255)));
    let unbounded = Interval::at_least(Integer::from(0));
    assert_eq!(int(-1).narrow(&unbounded, Narrowing::Wrap), Err(Unevaluated::trap(Reason::Narrowing)));
}
