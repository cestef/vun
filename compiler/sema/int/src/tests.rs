use super::{Divided, Divisor, Extended, Integer, Interval, Shift, Storage};

fn int(lo: i128, hi: i128) -> Interval {
    Interval::closed(lo.into(), hi.into()).unwrap()
}

/// Every result of applying `op` to the values of `a` and `b`, for checking ranges exhaustively.
fn exhaustive(a: &Interval, b: &Interval, op: impl Fn(i128, i128) -> Option<i128>) -> Option<Interval> {
    let bound = |e: &Extended| e.finite().and_then(Integer::to_i128).unwrap();
    let values = |r: &Interval| bound(r.lo())..=bound(r.hi());
    let op = &op;
    let results = values(a).flat_map(|x| values(b).filter_map(move |y| op(x, y)));
    results.fold(None, |range, v| {
        let point = Interval::singleton(v.into());
        Some(range.map_or(point.clone(), |r: Interval| r.hull(&point)))
    })
}

#[test]
fn arithmetic_is_exact_and_tight() {
    let samples = [int(-7, 5), int(0, 3), int(2, 9), int(-4, -1), int(0, 0), int(-3, 3)];
    for a in &samples {
        for b in &samples {
            assert_eq!(Some(a.add(b)), exhaustive(a, b, |x, y| Some(x + y)), "{a:?} + {b:?}");
            assert_eq!(Some(a.sub(b)), exhaustive(a, b, |x, y| Some(x - y)), "{a:?} - {b:?}");
            assert_eq!(Some(a.mul(b)), exhaustive(a, b, |x, y| Some(x * y)), "{a:?} * {b:?}");
            let div = exhaustive(a, b, i128::checked_div);
            assert_eq!(a.div(b).range, div, "{a:?} / {b:?}");
            // The remainder's bound is sound, and tight at the ends the dividend reaches.
            let rem = exhaustive(a, b, i128::checked_rem);
            if let (Some(bound), Some(exact)) = (a.rem(b).range, rem) {
                assert!(bound.contains(&exact), "{a:?} % {b:?}: {bound:?} misses {exact:?}");
            }
        }
    }
}

#[test]
fn a_divisor_that_may_be_zero_is_a_trap_site() {
    assert_eq!(int(1, 8).div(&int(2, 4)).divisor, Divisor::NonZero);
    assert_eq!(int(1, 8).div(&int(-1, 4)).divisor, Divisor::MayBeZero);
    assert_eq!(int(1, 8).div(&int(0, 0)), Divided { range: None, divisor: Divisor::MayBeZero });
}

#[test]
fn the_spec_examples_hold() {
    // `(a + b) / 2` for two u32 fits u32.
    let u32 = Interval::half_open(0.into(), &Integer::power_of_two(32)).unwrap();
    let mid = u32.add(&u32).div(&Interval::singleton(2.into())).range.unwrap();
    assert!(u32.contains(&mid));
    // `(head + 1) % 64` for `head: Int(0..64)` is `Int(0..64)`.
    let index = int(0, 63);
    let wrapped = index.add(&Interval::singleton(1.into())).rem(&Interval::singleton(64.into()));
    assert_eq!(wrapped.range, Some(index));
    // `|x| + |y|` for two i32 reaches 2^32, which u32 does not hold.
    let magnitude = Interval::closed(0.into(), Integer::power_of_two(31)).unwrap();
    assert!(!u32.contains(&magnitude.add(&magnitude)));
}

#[test]
fn storage_is_the_smallest_direct_width() {
    assert_eq!(int(0, 255).storage(), Some(Storage::B8));
    assert_eq!(int(0, 256).storage(), Some(Storage::B16));
    assert_eq!(int(-128, 127).storage(), Some(Storage::B8));
    assert_eq!(int(-129, 0).storage(), Some(Storage::B16));
    assert_eq!(int(-1, 200).storage(), Some(Storage::B16), "no offset encoding");
    assert_eq!(int(0, 7).bits(), Some(3));
    assert_eq!(int(-4, 3).bits(), Some(3));
    assert_eq!(int(-1, -1).bits(), Some(1));
    let u64 = Interval::half_open(0.into(), &Integer::power_of_two(64)).unwrap();
    assert_eq!(u64.mul(&u64).storage(), Some(Storage::B128));
    assert_eq!(u64.mul(&u64).mul(&u64).storage(), None, "past the width cap");
}

#[test]
fn unbounded_ranges_have_no_storage_and_stay_sound() {
    let from_one = Interval::at_least(1.into());
    assert_eq!(from_one.storage(), None, "compile time only");
    let shifted = from_one.sub(&Interval::singleton(1.into()));
    assert_eq!((shifted.lo(), shifted.hi()), (&Extended::Finite(0.into()), &Extended::PosInf));
    assert_eq!(int(-2, 3).mul(&from_one).lo(), &Extended::NegInf);
    assert_eq!(int(0, 0).mul(&from_one), int(0, 0), "zero times anything is zero");
    let quotient = int(10, 20).div(&from_one).range.unwrap();
    assert_eq!(quotient, int(0, 20));
    let remainder = int(-5, 5).rem(&from_one).range.unwrap();
    assert_eq!(remainder, int(-5, 5));
    assert!(from_one.contains(&int(1, 1_000_000)));
}

#[test]
fn shifts_are_exact() {
    let one = Integer::from(1);
    assert_eq!(one.shifted(Shift::Left, &64.into()), Some(Integer::power_of_two(64)));
    assert_eq!(Integer::from(-5).shifted(Shift::Right, &1.into()), Some(Integer::from(-3)), "rounds down");
    assert_eq!(Integer::from(-5).shifted(Shift::Left, &(-1).into()), Some(Integer::from(-3)));
    assert_eq!(one.shifted(Shift::Left, &Integer::power_of_two(40)), None);
    assert_eq!(Integer::from(-5).shifted(Shift::Right, &Integer::power_of_two(80)), Some(Integer::from(-1)));
    assert_eq!(Integer::zero().shifted(Shift::Left, &Integer::power_of_two(80)), Some(Integer::zero()));
}

#[test]
fn shifts_past_the_limit_have_no_range() {
    let amounts = int(0, i128::from(u32::MAX));
    assert_eq!(Interval::singleton(1.into()).shifted(&amounts, Shift::Left), None);
    let shifted = Interval::singleton(1.into()).shifted(&int(0, 8), Shift::Left).unwrap();
    assert_eq!(shifted, int(1, 256));
}
