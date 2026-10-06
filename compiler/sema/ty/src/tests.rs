use vun_primitives::Primitive;
use vun_primitives::mem::Mem;
use vun_sema_float::Format;
use vun_sema_int::{Integer, Interval};

use super::{TyKind, Value};
use crate::float::FloatType;

/// Every nested compile-time call keeps its arguments, locals and result alive, so the size of a
/// value is paid at each level: a function and a piece of code are shared, not held inline.
#[test]
fn a_value_is_small_where_each_nested_call_holds_several() {
    assert!(std::mem::size_of::<Value>() <= 80, "a value is {} bytes", std::mem::size_of::<Value>());
}

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

#[test]
fn borrows_of_arrays_and_slices_join_in_any_order() {
    let (two, three, four) =
        (TyKind::string(2).unwrap(), TyKind::string(3).unwrap(), TyKind::string(4).unwrap());
    let (mode, _, _) = two.borrowing().unwrap();
    let slice = TyKind::borrowed_as(mode, TyKind::slice(two.pointee().unwrap().items().unwrap()));
    assert_eq!(two.join(&three), slice);
    // The slice two arrays meet in meets a third array, whichever comes first.
    assert_eq!(two.join(&three).and_then(|s| s.join(&four)), slice);
    assert_eq!(four.join(&two.join(&three).unwrap()), slice);
}

#[test]
fn parts_are_read_by_borrowing_and_replaced_in_place() {
    use crate::ops::Part;
    let int = |n: i32| Value::Int(Integer::from(n));
    let mut grid = Value::Array(vec![Value::Tuple(vec![int(1), int(2)]), Value::Tuple(vec![int(3), int(4)])]);
    let second = [Part::Item(int(1)), Part::Position(0)];
    assert_eq!(*grid.at(&second).unwrap(), int(3));
    grid.replace(&second, int(9)).unwrap();
    assert_eq!(*grid.at(&second).unwrap(), int(9));
    assert_eq!(*grid.at(&[Part::Item(int(0)), Part::Position(1)]).unwrap(), int(2));
    // An index past the end traps, and changes nothing.
    let before = grid.clone();
    assert!(grid.replace(&[Part::Item(int(2)), Part::Position(0)], int(0)).is_err());
    assert_eq!(grid, before);
}
