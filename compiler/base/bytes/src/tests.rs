use super::{Out, Strings};

#[test]
fn leb128_encodes_as_readers_read_it() {
    let mut out = Out::default();
    out.uleb(624_485);
    out.sleb(-123_456);
    out.sleb(63);
    out.sleb(-64);
    out.sleb(64);
    assert_eq!(out.0, [0xe5, 0x8e, 0x26, 0xc0, 0xbb, 0x78, 0x3f, 0x40, 0xc0, 0x00]);
}

#[test]
fn padded_leb128_takes_five_bytes_and_reads_the_same() {
    let mut out = Out::default();
    out.uleb_padded(3);
    out.sleb_padded(-2);
    assert_eq!(out.0, [0x83, 0x80, 0x80, 0x80, 0x00, 0xfe, 0xff, 0xff, 0xff, 0x7f]);
}

#[test]
fn a_string_table_starts_with_the_empty_string_and_names_each_by_its_start() {
    let mut strings = Strings::default();
    assert_eq!((strings.add(b"ab"), strings.add(b"")), (1, 4));
    assert_eq!(strings.into_bytes(), b"\0ab\0\0");
    assert_eq!(Strings::default().into_bytes(), [0]);
}

#[test]
fn padded_fields_are_rewritten_in_place() {
    let mut out = Out::default();
    out.u8(0xaa);
    out.uleb_padded(0);
    out.sleb_padded(0);
    out.u32(0);
    out.rewrite_uleb_padded(1, 3);
    out.rewrite_sleb_padded(6, -2);
    out.rewrite_u32(11, 0x0102_0304);
    let mut expected = Out::default();
    expected.u8(0xaa);
    expected.uleb_padded(3);
    expected.sleb_padded(-2);
    expected.u32(0x0102_0304);
    assert_eq!(out, expected);
}

#[test]
fn what_out_writes_in_reads_back_and_every_read_is_checked() {
    let mut out = Out::default();
    out.u8(7);
    out.u16(0x1234);
    out.u32(0xdead_beef);
    out.u64(1 << 40);
    out.str(b"name");
    let mut input = super::In::at(&out.0, 0);
    assert_eq!(input.u8(), Ok(7));
    assert_eq!(input.u16(), Ok(0x1234));
    assert_eq!(input.u32(), Ok(0xdead_beef));
    assert_eq!(input.u64(), Ok(1 << 40));
    assert_eq!(super::In::string(&out.0, input.position()), Ok(&b"name"[..]));
    assert_eq!(input.take(5), Ok(&b"name\0"[..]));
    assert_eq!(input.u8(), Err(super::Truncated { at: 1 + out.0.len() as u64, end: out.0.len() as u64 }));
    assert!(super::In::string(&out.0[..out.0.len() - 1], 15).is_err());
}

#[test]
fn a_name_is_ended_by_a_zero_within_the_longest_a_reader_copies() {
    use super::{In, NAME_MAX};
    let mut table = vec![0u8];
    table.extend(b"short\0");
    assert_eq!(In::name(&table, 1), Some(&b"short"[..]));
    assert_eq!(In::name(&table, 0), Some(&b""[..]), "the empty string");
    assert_eq!(In::name(&table, 7), None, "past the table");
    assert_eq!(In::name(&table, u64::MAX), None);
    let mut long = vec![b'a'; NAME_MAX];
    long.push(0);
    assert_eq!(In::name(&long, 0).map(<[u8]>::len), Some(NAME_MAX));
    let mut longer = vec![b'a'; NAME_MAX + 1];
    longer.push(0);
    assert_eq!(In::name(&longer, 0), None, "one byte past the longest");
    assert_eq!(In::name(&longer[..NAME_MAX], 0), None, "no zero ends it");
}

#[test]
fn copies_are_spent_until_none_are_left() {
    use super::{Copies, Overspent};
    let mut copies = Copies::of(10);
    assert_eq!(copies.spend(4), Ok(()));
    assert_eq!(copies.spend(6), Ok(()));
    assert_eq!(copies.spend(1), Err(Overspent));
    assert_eq!(copies.spend(0), Ok(()));
}
