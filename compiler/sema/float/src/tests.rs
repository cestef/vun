use std::cmp::Ordering;

use super::{Binary, Format};

fn parsed(text: &str, format: Format) -> u64 {
    Binary::parse(text, format).unwrap().bits
}

fn f64_of(text: &str) -> Binary {
    Binary::parse(text, Format::F64).unwrap()
}

#[test]
fn decimals_round_to_nearest_even_in_each_format() {
    assert_eq!(parsed("0.1", Format::F64), 0.1f64.to_bits());
    assert_eq!(parsed("0.1", Format::F32), u64::from(0.1f32.to_bits()));
    assert_eq!(parsed("0.1", Format::F16), 0x2e66);
    assert_eq!(parsed("1_000.5e-2", Format::F64), 10.005f64.to_bits());
    assert_eq!(parsed("65520", Format::F16), 0x7c00);
    assert_eq!(parsed("5e-324", Format::F64), 1);
    assert_eq!(parsed("1e-400", Format::F64), 0);
    assert_eq!(parsed("1e1000000000", Format::F64), f64::INFINITY.to_bits());
    assert_eq!(parsed("1e-1000000000", Format::F64), 0);
    assert_eq!(parsed("0e1000000000", Format::F64), 0);
    assert_eq!(Binary::parse("1.5e-9223372036854775808", Format::F64), None);
    assert_eq!(Binary::parse("", Format::F64), None);
    assert_eq!(Binary::parse(".", Format::F64), None);
    assert_eq!(parsed("3.4028235e38", Format::F32), u64::from(f32::MAX.to_bits()));
}

#[test]
fn arithmetic_rounds_the_exact_result_once() {
    let (a, b) = (f64_of("0.1"), f64_of("0.2"));
    assert_eq!((a + b).bits, (0.1f64 + 0.2f64).to_bits());
    assert_eq!((a * b).bits, (0.1f64 * 0.2f64).to_bits());
    assert_eq!((a / b).bits, (0.1f64 / 0.2f64).to_bits());
    assert_eq!((a - a).bits, 0);
    assert_eq!((f64_of("5.5") % f64_of("2")).bits, 1.5f64.to_bits());
    assert_eq!(a.mul_add(b, f64_of("0.3")).bits, 0.1f64.mul_add(0.2, 0.3).to_bits());
    let zero = Binary::zero(Format::F64, false);
    assert_eq!(f64_of("1") / zero, Binary::infinity(Format::F64, false));
    assert_eq!(zero / zero, Binary::nan(Format::F64));
}

#[test]
fn comparisons_leave_nans_unordered_and_total_order_ranks_them() {
    let nan = Binary::nan(Format::F32);
    let (zero, one) = (Binary::zero(Format::F32, false), Binary::parse("1", Format::F32).unwrap());
    assert_eq!(nan.compare(one), None);
    assert_eq!(zero.compare(-zero), Some(Ordering::Equal));
    assert_eq!((-one).compare(zero), Some(Ordering::Less));
    assert_eq!((-zero).total_order(zero), Ordering::Less);
    assert_eq!(one.total_order(nan), Ordering::Less);
    assert_eq!((-nan).total_order(-one), Ordering::Less);
}
