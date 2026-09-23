use std::sync::Arc;

use crate::{Decode, Encode, Malformed, record};

fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(value: &T) {
    assert_eq!(&T::decoded(&value.encoded()).unwrap(), value);
}

#[derive(Debug, PartialEq)]
struct Memo {
    key: String,
    deps: Vec<(u32, [u8; 4])>,
    hole: Option<i128>,
}
record!(Memo { key, deps, hole });

#[test]
fn values_round_trip() {
    for n in [0, 1, 127, 128, 300, u64::MAX] {
        round_trip(&n);
    }
    for n in [0, -1, 1, -64, 64, i128::MIN, i128::MAX] {
        round_trip(&n);
    }
    round_trip(&u128::MAX);
    round_trip(&(true, String::from("é"), vec![Some(3_u16), None]));
    round_trip(&Arc::new(7_u32));
    round_trip(&Memo { key: "a".into(), deps: vec![(1, [1, 2, 3, 4])], hole: Some(-5) });
    assert_eq!(0_u64.encoded(), [0]);
    assert_eq!(300_u64.encoded(), [0xac, 0x02], "LEB128");
    assert_eq!((-1_i128).encoded(), [1], "zigzag");
}

#[test]
fn malformed_input_is_named() {
    assert_eq!(u32::decoded(&[0x80]), Err(Malformed::Truncated));
    assert_eq!(u8::decoded(&[0xff; 6].map(|b| b)).map(|_| ()), Err(Malformed::Trailing));
    assert_eq!(u16::decoded(&70_000_u32.encoded()), Err(Malformed::Overflow));
    assert_eq!(bool::decoded(&[2]), Err(Malformed::Tag(2)));
    assert_eq!(String::decoded(&[1, 0xff]), Err(Malformed::Text));
    assert_eq!(Vec::<u8>::decoded(&[9, 1]), Err(Malformed::Truncated));
    assert_eq!(u128::decoded(&[0xff; 20]), Err(Malformed::Overflow));
}
