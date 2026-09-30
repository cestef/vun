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
