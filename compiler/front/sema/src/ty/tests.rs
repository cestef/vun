use super::{TyKind, Value};
use crate::Primitive;
use crate::int::{Integer, Interval};
use crate::primitives::{lang::Lang, mem::Mem};

#[test]
fn equal_types_are_one_and_ranges_are_sets() {
    let u8 = TyKind::int(&Interval::closed(0.into(), 255.into()).unwrap());
    let half_open = TyKind::int(&Interval::half_open(0.into(), &256.into()).unwrap());
    assert_eq!(u8, half_open, "`Int(0..256)` is `Int(0..=255)`");
    assert_eq!(u8.interval(), Interval::closed(0.into(), 255.into()));
    let byte_ref = TyKind::Primitive(Primitive::Mem(Mem::Ref), vec![Value::Type(u8)]).intern();
    assert_ne!(byte_ref, u8);
    assert_eq!(TyKind::primitive(Primitive::Lang(Lang::f32)), TyKind::primitive(Primitive::Lang(Lang::f32)));
    let array = TyKind::array(Integer::from(4), u8);
    let hash = |t| vun_hash::Hash::of_value(&t);
    assert_eq!(hash(array), hash(TyKind::array(Integer::from(4), half_open)));
}
