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
