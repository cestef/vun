//! Checks of unsafe code at run time (spec 07, section 7): under the program's checks, an
//! operation marked `#[lang.check.by]` runs as its checked version, a trap site of its own.

use vun_diag::{DiagId, errors};
use vun_intern::Symbol;
use vun_sema::target::Choice;
use vun_target::Check;

use crate::common::Fixture;

#[test]
fn a_raw_read_runs_as_its_checked_version_under_the_pointer_checks() {
    let fixture = Fixture::new("checks-pointers");
    let path = fixture.root.write(
        "p.vun",
        "\
#[link.export(\"first\")]
first := (xs: &[4]u8) -> u8 { unsafe lang.mem.pointer_to([4]u8, xs).cast(u8).read() };
",
    );
    let lowered = |checks: &[Check]| {
        let mut session = fixture.session();
        session.add_checks(checks.iter().copied());
        let file = session.load(&path).unwrap();
        let unit = session.lir(file).as_ref().clone().unwrap();
        assert_eq!(unit.verify(session.origins), Ok(()));
        let imports: Vec<Symbol> = unit.imports.iter().map(|(_, i)| i.symbol).collect();
        (unit.sites.len(), imports)
    };
    let (sites, imports) = lowered(&[]);
    let (checked, checked_imports) = lowered(&[Check::Pointers]);
    assert_eq!(checked, sites + 1);
    for symbol in ["vun_check_access", "vun_check_fail"] {
        assert!(!imports.contains(&Symbol::intern(symbol)));
        assert!(checked_imports.contains(&Symbol::intern(symbol)), "{checked_imports:?}");
    }
}

/// A program reading an item of an array through a raw pointer.
const THIRD: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    xs: [4]u8 = [1, 2, 3, 4];
    third := unsafe lang.mem.pointer_to([4]u8, &xs).cast(u8).offset(2).read();
    std.os.Exit.of((third + 7).to(Int(0..=125)))
};
";

#[test]
fn a_checked_program_runs_through_each_backend_over_its_runtime_shadow() {
    let fixture = Fixture::new("checks-run");
    let path = fixture.root.write("main.vun", THIRD);
    let mut session = fixture.session();
    session.add_checks([Check::Pointers]);
    let file = session.load(&path).unwrap();
    let runs = session.run_each(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let statuses: Vec<Option<i32>> = runs.iter().map(|r| r.output.status.code()).collect();
    assert!(statuses.len() > 1 && statuses.iter().all(|&s| s == Some(10)), "{statuses:?}");
}

#[test]
fn a_runtime_without_the_operations_of_the_checks_is_refused() {
    let fixture = Fixture::new("checks-unsupported");
    let path = fixture.root.write("main.vun", THIRD);
    let mut session = fixture.session();
    session.choice = Choice::Path(vec![Symbol::intern("wasi"), Symbol::intern("wasm32")]);
    session.add_checks([Check::Pointers]);
    let file = session.load(&path).unwrap();
    let _ = session.compile(file);
    let ids: Vec<DiagId> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::CHECK_UNSUPPORTED]);
}
