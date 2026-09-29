use std::io::Cursor;

use vun_lsp_protocol::base::PositionEncoding;

use crate::position::Lines;
use crate::transport::Incoming;

#[test]
fn positions_count_bytes_or_utf16_units_or_scalars() {
    let text = "a := \"é𝄞\";\nb := 1;\n";
    let lines = Lines::of(text);
    let b = u32::try_from(text.find('b').unwrap()).unwrap();
    let end = u32::try_from(text.find(';').unwrap()).unwrap();
    let encodings = [PositionEncoding::Utf8, PositionEncoding::Utf16, PositionEncoding::Utf32];
    for encoding in encodings {
        for offset in [0, b, end] {
            let position = lines.position(text, offset, encoding);
            assert_eq!(lines.offset(text, position, encoding), Some(offset), "{encoding:?} {offset}");
        }
    }
    let semicolon = encodings.map(|e| lines.position(text, end, e).character);
    assert_eq!(semicolon, [13, 10, 9]);
}

#[test]
fn messages_are_read_by_their_length() {
    let framed = "Content-Length: 2\r\n\r\n{}content-length: 4\r\nOther: x\r\n\r\nnull";
    let mut incoming = Incoming(Cursor::new(framed));
    assert_eq!(incoming.next().unwrap().as_deref(), Some("{}"));
    assert_eq!(incoming.next().unwrap().as_deref(), Some("null"));
    assert_eq!(incoming.next().unwrap(), None);
}
