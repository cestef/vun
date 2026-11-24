//! The measures of `vun check` (spec 04, section 1; spec 03, section 8): wide intermediates and the
//! code of generics, reported only when asked for.

use crate::common::Fixture;
use vun_driver::reports::{Report, Reports};

const PROGRAM: &str = "\
square := (x: u64) -> u128 { x * x };
first := ($T: type, x: T) -> T { x };
main := () -> u8 { _ := square(3); a := first(u8, 1); b := first(u16, 2); a };
";

fn widths(name: &str, asked: &[Report]) -> Vec<(u64, u32)> {
    let fixture = Fixture::new(name);
    let file = fixture.root.write("measure.vun", PROGRAM);
    let mut session = fixture.session();
    session.reports = Reports::asking(asked.iter().copied());
    let file = session.load(&file).unwrap();
    session.check(file);
    session.measured(file);
    session.reports.widths.iter().map(|w| (w.bits, w.register)).collect()
}

#[test]
fn a_wide_intermediate_is_reported_only_when_asked() {
    assert!(widths("widths-off", &[]).is_empty());
    let asked = widths("widths-on", &[Report::Widths]);
    assert!(asked.iter().any(|&(bits, register)| bits == 128 && register <= 64), "{asked:?}");
}

#[test]
fn a_generic_is_reported_with_its_instantiations() {
    let fixture = Fixture::new("generics");
    let file = fixture.root.write("measure.vun", PROGRAM);
    let mut session = fixture.session();
    session.reports = Reports::asking([Report::Generics]);
    let file = session.load(&file).unwrap();
    session.check(file);
    session.measured(file);
    let first = session.reports.generics.iter().find(|g| g.name.as_deref() == Some("first")).unwrap();
    assert_eq!(first.instantiations, 2);
    assert!(first.statements > 0);
}
