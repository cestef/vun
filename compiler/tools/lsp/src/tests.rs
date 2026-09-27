use std::io::Cursor;
use std::path::Path;

use vun_driver::Session;
use vun_fs::Scratch;
use vun_json::{Object, Value};

use crate::message::Outgoing;
use crate::position::{Encoding, Lines};
use crate::server::Server;
use crate::transport::Incoming;
use crate::uri::Uri;

#[test]
fn positions_count_bytes_or_utf16_units() {
    let text = "a := \"é𝄞\";\nb := 1;\n";
    let lines = Lines::of(text);
    let b = u32::try_from(text.find('b').unwrap()).unwrap();
    let end = u32::try_from(text.find(';').unwrap()).unwrap();
    for encoding in [Encoding::Utf8, Encoding::Utf16] {
        for offset in [0, b, end] {
            let position = lines.position(text, offset, encoding);
            assert_eq!(lines.offset(text, &position, encoding), Some(offset), "{encoding:?} {offset}");
        }
    }
    let semicolon = |e| lines.position(text, end, e)["character"].as_u64();
    assert_eq!((semicolon(Encoding::Utf8), semicolon(Encoding::Utf16)), (Some(13), Some(10)));
}

#[test]
fn uris_escape_what_paths_hold() {
    let path = Path::new("/home/a b/ü.vun");
    let uri = Uri::of(path);
    assert_eq!(uri.as_str(), "file:///home/a%20b/%C3%BC.vun");
    assert_eq!(uri.path().as_deref(), Some(path));
    assert_eq!(Uri::new("untitled:1").path(), None);
}

#[test]
fn messages_are_read_by_their_length() {
    let framed = "Content-Length: 2\r\n\r\n{}content-length: 4\r\nOther: x\r\n\r\nnull";
    let mut incoming = Incoming(Cursor::new(framed));
    assert_eq!(incoming.next().unwrap().as_deref(), Some("{}"));
    assert_eq!(incoming.next().unwrap().as_deref(), Some("null"));
    assert_eq!(incoming.next().unwrap(), None);
}

/// A client of `server`: each message it sends, and what the server answered or published.
struct Client<'s, 'd> {
    server: &'s mut Server<'d>,
    next: i64,
}

impl Client<'_, '_> {
    fn request(&mut self, method: &str, params: Object<'static>) -> Value<'static> {
        self.next += 1;
        let message = Object::new().with("jsonrpc", "2.0").with("id", self.next).with("method", method);
        let reply = self.server.handle(&Value::from(message.with("params", params)).to_string());
        reply.map(Outgoing::value).unwrap_or_default()
    }

    fn notify(&mut self, method: &str, params: Object<'static>) {
        let message = Object::new().with("jsonrpc", "2.0").with("method", method).with("params", params);
        assert!(self.server.handle(&Value::from(message).to_string()).is_none());
    }

    /// The diagnostics published since the last call, by document.
    fn published(&mut self) -> Vec<Value<'static>> {
        self.server.publish();
        self.server.outbox.drain(..).map(|m| m.value()["params"].clone()).collect()
    }
}

fn at(uri: &Uri, line: u32, character: u32) -> Object<'static> {
    let position = Object::new().with("line", line).with("character", character);
    Object::new()
        .with("textDocument", Object::new().with("uri", uri.as_str().to_owned()))
        .with("position", position)
}

#[test]
fn an_open_document_is_checked_hovered_and_followed_per_version() {
    let dir = Scratch::new("lsp-session");
    let path = std::path::absolute(dir.write("a.vun", "")).unwrap();
    let uri = Uri::of(&path);
    let mut session = Session::new();
    let mut server = Server::new(&mut session);
    let mut client = Client { server: &mut server, next: 0 };

    let init = client.request("textDocument/hover", Object::new());
    assert_eq!(init["error"]["code"].as_i64(), Some(-32002), "{init}");
    let encodings = Object::new().with("positionEncodings", vec![Value::from("utf-8")]);
    let init = client
        .request("initialize", Object::new().with("capabilities", Object::new().with("general", encodings)));
    assert_eq!(init["result"]["capabilities"]["positionEncoding"].as_str(), Some("utf-8"), "{init}");
    assert_eq!(init["result"]["capabilities"]["hoverProvider"].as_bool(), Some(true));
    client.notify("initialized", Object::new());

    let text = "/// The limit.\nx: u8 = 300;\nf := () -> u8 { x };\n";
    let document = Object::new().with("uri", uri.as_str().to_owned()).with("version", 1).with("text", text);
    client.notify("textDocument/didOpen", Object::new().with("textDocument", document));
    let published = client.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0]["version"].as_i64(), Some(1));
    let codes: Vec<_> = published[0]["diagnostics"].items().filter_map(|d| d["code"].as_str()).collect();
    assert_eq!(codes, ["int.does_not_fit"], "{}", published[0]);

    let fixed = "/// The limit.\nx: u8 = 200;\nf := () -> u8 { x };\n";
    let change = Object::new()
        .with("textDocument", Object::new().with("uri", uri.as_str().to_owned()).with("version", 2));
    client.notify(
        "textDocument/didChange",
        change.with("contentChanges", vec![Value::from(Object::new().with("text", fixed))]),
    );
    let published = client.published();
    assert_eq!((published[0]["version"].as_i64(), published[0]["diagnostics"].items().len()), (Some(2), 0));

    let hover = client.request("textDocument/hover", at(&uri, 2, 16));
    let shown = hover["result"]["contents"]["value"].as_str().unwrap_or_default();
    assert!(shown.contains("Int(0..=255)") && shown.contains("The limit."), "{hover}");
    let definition = client.request("textDocument/definition", at(&uri, 2, 16));
    assert_eq!(definition["result"]["uri"].as_str(), Some(uri.as_str()), "{definition}");
    assert_eq!(definition["result"]["range"]["start"]["line"].as_u64(), Some(1));

    let unknown = client.request("textDocument/rename", Object::new());
    assert_eq!(unknown["error"]["code"].as_i64(), Some(-32601));
    client.request("shutdown", Object::new());
    let late = client.request("textDocument/hover", at(&uri, 2, 16));
    assert_eq!(late["error"]["code"].as_i64(), Some(-32600));
}
