use std::sync::{Arc, Mutex};

use crate::{Context, Decode, Encode, Malformed, Pooled, Reader, Resources, Tables, Writer};

fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(value: &T) {
    assert_eq!(&T::decoded(&value.encoded()).unwrap(), value);
}

#[derive(Debug, PartialEq, Encode, Decode)]
struct Memo {
    key: String,
    deps: Vec<(u32, [u8; 4])>,
    hole: Option<i128>,
}

#[derive(Debug, PartialEq, Encode, Decode)]
enum Shape<T, const N: usize> {
    Unit,
    Tuple(T, [u8; N]),
    Named { first: T, rest: Vec<T> },
}

#[derive(Debug, PartialEq, Encode, Decode)]
struct Wrapper<'a, T: Clone = u8>(Box<T>, std::marker::PhantomData<&'a ()>)
where
    T: Copy;

#[derive(Debug, PartialEq, Encode, Decode)]
struct Empty;

#[test]
fn values_round_trip() {
    for n in [0, 1, 127, 128, 300, u64::MAX] {
        round_trip(&n);
    }
    for n in [0, -1, 1, -64, 64, i128::MIN, i128::MAX] {
        round_trip(&n);
    }
    round_trip(&i32::MIN);
    round_trip(&u128::MAX);
    round_trip(&(true, String::from("é"), vec![Some(3_u16), None]));
    round_trip(&Arc::new(7_u32));
    round_trip(&Arc::<[u8]>::from([1, 2]));
    round_trip(&Result::<u8, String>::Err("no".into()));
    round_trip(&Memo { key: "a".into(), deps: vec![(1, [1, 2, 3, 4])], hole: Some(-5) });
    assert_eq!(0_u64.encoded(), [0]);
    assert_eq!(300_u64.encoded(), [0xac, 0x02], "LEB128");
    assert_eq!((-1_i128).encoded(), [1], "zigzag");
}

#[test]
fn derived_shapes_round_trip() {
    round_trip(&Shape::<u16, 2>::Unit);
    round_trip(&Shape::<u16, 2>::Tuple(9, [1, 2]));
    round_trip(&Shape::<u16, 2>::Named { first: 1, rest: vec![2, 3] });
    round_trip(&Wrapper::<u8>(Box::new(4), std::marker::PhantomData));
    round_trip(&Empty);
    assert_eq!(Shape::<u8, 0>::Named { first: 5, rest: vec![] }.encoded(), [2, 5, 0], "tag, then fields");
    assert_eq!(Empty.encoded(), [0_u8; 0]);
    assert_eq!(Shape::<u8, 0>::decoded(&[3]), Err(Malformed::Tag(3)));
}

#[test]
fn malformed_input_is_named() {
    assert_eq!(u32::decoded(&[0x80]), Err(Malformed::Truncated));
    assert_eq!(u8::decoded(&[0xff; 6]).map(|_| ()), Err(Malformed::Trailing));
    assert_eq!(u16::decoded(&70_000_u32.encoded()), Err(Malformed::Overflow));
    assert_eq!(bool::decoded(&[2]), Err(Malformed::Tag(2)));
    assert_eq!(String::decoded(&[1, 0xff]), Err(Malformed::Text));
    assert_eq!(Vec::<u8>::decoded(&[9, 1]), Err(Malformed::Truncated));
    assert_eq!(u128::decoded(&[0xff; 20]), Err(Malformed::Overflow));
}

/// A process's table of names, handed out as indexes.
#[derive(Default)]
struct Names(Mutex<Vec<String>>);

impl Names {
    fn add(&self, name: &str) -> Name {
        let mut names = self.0.lock().unwrap();
        names.push(name.to_owned());
        Name(names.len() - 1)
    }
}

/// An index into the process's [`Names`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Name(usize);

impl Pooled for Name {
    const POOL: &'static str = "name";
    type Table = Names;

    fn write(self, table: &Names, w: &mut Writer<'_>) {
        table.0.lock().unwrap()[self.0].encode(w);
    }

    fn read(table: &Names, r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(table.add(&String::decode(r)?))
    }
}

crate::pooled!(Name);

#[test]
fn pooled_handles_are_written_once_and_recorded_once() {
    let writing = Tables::default();
    let names = (&writing as &dyn Resources).get::<Names>();
    let (a, b) = (names.add("a"), names.add("b"));
    let mut w = Writer::pooling(&writing);
    let first = w.encode(&vec![b, a, b]);
    let second = w.encode(&a);
    let pools = w.into_pools();
    assert_eq!(pools.0[0].entries, [b"\x01b".to_vec(), b"\x01a".to_vec()], "in the order first written");
    assert_eq!(first, [3, 0, 1, 0], "by number");

    // Another process's table already holds a name: the handles read are its own.
    let reading = Tables::default();
    (&reading as &dyn Resources).get::<Names>().add("other");
    let context = Context::new(&pools, &reading);
    assert_eq!(context.read::<Vec<Name>>(&first).unwrap(), [Name(1), Name(2), Name(1)]);
    assert_eq!(context.read::<Name>(&second).unwrap(), Name(2), "recorded once per stream");
    assert_eq!(context.read::<Name>(&[7]), Err(Malformed::Unpooled { pool: "name", number: 7 }));
}
