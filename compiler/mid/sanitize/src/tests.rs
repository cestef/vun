use vun_target::Check;

use crate::Checks;

#[test]
fn checks_hold_each_kind_once_in_order() {
    let checks = Checks::of([Check::Asm, Check::Pointers, Check::Asm]);
    assert_eq!(checks, Checks::of([Check::Pointers, Check::Asm]));
    assert!(checks.contains(Check::Asm) && !checks.contains(Check::Allocations));
    assert!(Checks::default().is_empty());
}
