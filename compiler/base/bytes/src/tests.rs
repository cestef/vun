use super::Out;

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
