use crate::{Checks, Kind};

#[test]
fn checks_hold_each_kind_once_in_order() {
    let checks = Checks::of([Kind::Asm, Kind::Pointers, Kind::Asm]);
    assert_eq!(checks, Checks::of([Kind::Pointers, Kind::Asm]));
    assert!(checks.contains(Kind::Asm) && !checks.contains(Kind::Allocations));
    assert!(Checks::default().is_empty());
}
