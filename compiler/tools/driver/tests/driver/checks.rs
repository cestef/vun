//! Checks of unsafe code at run time (spec 07, section 7): under the program's checks, an
//! operation marked `#[lang.check.by]` runs as its checked version, a trap site of its own.

use vun_intern::Symbol;
use vun_sanitize::{Checks, ChecksOf, Kind};

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
    let lowered = |checks: Checks| {
        let mut session = fixture.session();
        session.db.set::<ChecksOf>(&(), checks);
        let file = session.load(&path).unwrap();
        let unit = session.lir(file).as_ref().clone().unwrap();
        assert_eq!(unit.verify(session.origins), Ok(()));
        let imports: Vec<Symbol> = unit.imports.iter().map(|(_, i)| i.symbol).collect();
        (unit.sites.len(), imports)
    };
    let (sites, imports) = lowered(Checks::default());
    let (checked, checked_imports) = lowered(Checks::of([Kind::Pointers]));
    assert_eq!(checked, sites + 1);
    for symbol in ["vun_check_access", "vun_check_fail"] {
        assert!(!imports.contains(&Symbol::intern(symbol)));
        assert!(checked_imports.contains(&Symbol::intern(symbol)), "{checked_imports:?}");
    }
}
