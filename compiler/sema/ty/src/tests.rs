use vun_primitives::Primitive;
use vun_primitives::mem::Mem;
use vun_sema_float::Format;
use vun_sema_int::{Integer, Interval};

use super::{TyKind, Value};
use crate::float::FloatType;

#[test]
fn equal_types_are_one_and_ranges_are_sets() {
    let u8 = TyKind::int(&Interval::closed(0.into(), 255.into()).unwrap());
    let half_open = TyKind::int(&Interval::half_open(0.into(), &256.into()).unwrap());
    assert_eq!(u8, half_open, "`Int(0..256)` is `Int(0..=255)`");
    assert_eq!(u8.interval(), Interval::closed(0.into(), 255.into()));
    let byte_ref = TyKind::Primitive(Primitive::Mem(Mem::Ref), vec![Value::Type(u8)]).intern();
    assert_ne!(byte_ref, u8);
    assert_eq!(Format::F32.ty(), Format::F32.ty());
    let array = TyKind::array(Integer::from(4), u8);
    let hash = |t| vun_hash::Hash::of_value(&t);
    assert_eq!(hash(array), hash(TyKind::array(Integer::from(4), half_open)));
}
