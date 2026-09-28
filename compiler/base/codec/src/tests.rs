use std::sync::{Arc, Mutex};

use crate::{Context, Decode, Encode, Malformed, Pooled, Reader, Writer};

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

/// The process's table of names, handed out as indexes.
static NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// An index into [`NAMES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Name(usize);

impl Name {
    fn add(name: &str) -> Self {
        let mut names = NAMES.lock().unwrap();
        names.push(name.to_owned());
        Self(names.len() - 1)
    }

    fn text(self) -> String {
        NAMES.lock().unwrap()[self.0].clone()
    }
}

impl Pooled for Name {
    const POOL: &'static str = "name";

    fn write(self, w: &mut Writer<'_>) {
        self.text().encode(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Ok(Self::add(&String::decode(r)?))
    }
}

crate::pooled!(Name);

#[test]
fn pooled_handles_are_written_once_and_recorded_once() {
    let (a, b) = (Name::add("a"), Name::add("b"));
    let mut w = Writer::new();
    let first = w.encode(&vec![b, a, b]);
    let second = w.encode(&a);
    let pools = w.into_pools();
    assert_eq!(pools.0[0].entries, [b"\x01b".to_vec(), b"\x01a".to_vec()], "in the order first written");
    assert_eq!(first, [3, 0, 1, 0], "by number");

    // Read back, each entry is recorded once, as handles of its own.
    let context = Context::new(pools);
    let read: Vec<Name> = context.read(&first).unwrap();
    assert_eq!(read.iter().map(|n| n.text()).collect::<Vec<_>>(), ["b", "a", "b"]);
    assert_eq!((read[0], read[2]), (read[2], read[0]));
    assert_ne!(read[1], a, "recorded anew");
    assert_eq!(context.read::<Name>(&second).unwrap(), read[1], "recorded once per stream");
    assert_eq!(context.read::<Name>(&[7]), Err(Malformed::Unpooled { pool: "name", number: 7 }));
}

#[test]
fn a_stream_extended_keeps_the_bytes_of_its_values() {
    let (a, b) = (Name::add("a"), Name::add("b"));
    let mut w = Writer::new();
    let (old_a, old_b) = (w.encode(&a), w.encode(&b));
    let stream = Context::new(w.into_pools());

    // The next process reads one value, keeps the other's bytes, and writes a value of its own.
    let read: Name = stream.read(&old_a).unwrap();
    let c = Name::add("c");
    let mut w = Writer::extending(&stream);
    let (again, new) = (w.encode(&read), w.encode(&c));
    assert_eq!(again, old_a, "a handle read from the stream keeps its number");
    let extended = Context::new(w.into_pools());
    let texts: Vec<String> =
        [&old_b, &new].iter().map(|bytes| extended.read::<Name>(bytes).unwrap().text()).collect();
    assert_eq!(texts, ["b", "c"], "bytes of the old stream read the same in the new one");
}

#[test]
fn blobs_are_read_in_place() {
    use crate::Blob;

    let bytes = (7_u8, vec![1_u8, 2, 3], String::from("x")).encoded();
    let (seven, blob, x) = Context::default().read::<(u8, Blob, String)>(&bytes).unwrap();
    assert_eq!((seven, blob, x.as_str()), (7, Blob { at: 2, len: 3 }, "x"));
    assert_eq!(blob.of(&bytes), [1, 2, 3], "written as bytes, read as their place");
}

/// A handle whose entry may name another handle of its pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Chain;

impl Pooled for Chain {
    const POOL: &'static str = "chain";

    fn write(self, _: &mut Writer<'_>) {}

    fn read(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        if bool::decode(r)? { r.pooled::<Self>() } else { Ok(Self) }
    }
}

crate::pooled!(Chain);

#[test]
fn entries_referring_to_themselves_are_malformed() {
    let pools = |entries| crate::Pools(vec![crate::Pool { name: Chain::POOL.to_owned(), entries }]);
    let ended = Context::new(pools(vec![vec![1, 1], vec![0]]));
    assert_eq!(ended.read::<Chain>(&[0]), Ok(Chain));
    let looping = Context::new(pools(vec![vec![1, 1], vec![1, 0]]));
    assert_eq!(looping.read::<Chain>(&[0]), Err(Malformed::Cycle));
}

#[test]
fn a_layer_names_the_handles_of_the_stream_under_it() {
    let (a, b) = (Name::add("a"), Name::add("b"));
    let mut w = Writer::new();
    let old_a = w.encode(&a);
    let base = std::sync::Arc::new(Context::new(w.into_pools()));
    let read: Name = base.read(&old_a).unwrap();

    let mut w = Writer::over(&base);
    let (again, new) = (w.encode(&read), w.encode(&b));
    assert_eq!(again, old_a, "a handle of the stream under keeps its number");
    let pools = w.into_pools();
    assert_eq!(pools.0.iter().map(|p| p.entries.len()).sum::<usize>(), 1, "written without the base's");
    let (input, pools) = pools.placed();
    let layer = Context::over(base, input, pools);
    let texts: Vec<String> =
        [&old_a, &new].iter().map(|bytes| layer.read::<Name>(bytes).unwrap().text()).collect();
    assert_eq!(texts, ["a", "b"]);
}
