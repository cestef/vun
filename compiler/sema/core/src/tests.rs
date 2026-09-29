/// A database holding a `lang` with an integer primitive and an app seeing it through its
/// prelude, the app's root file holding `app`.
pub(crate) fn packages(app: &str) -> (vun_query::Db, vun_hir::db::FileRef) {
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
    let reports = db.get::<crate::check::Checked>(&file).unwrap();
    let messages: Vec<String> =
        reports.sorted(vun_origin::OriginTable::global()).iter().map(|d| d.message.to_string()).collect();
    assert_eq!(
        messages,
        ["expected `bool`, found `Int(3..=3)`", "value does not fit the type `u8`"],
        "{reports:?}"
    );
}

#[test]
fn narrowing_traps_wraps_and_saturates() {
    use vun_desugar::Narrowing;
    use vun_sema_int::{Integer, Interval};
    use vun_sema_ty::Value;
    use vun_sema_ty::trap::Reason;
    use vun_sema_ty::trap::Unevaluated;

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
