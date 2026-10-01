use vun_data::Value;
use vun_driver::Session;
use vun_fs::Scratch;
use vun_lsp_protocol::base::{Position, PositionEncoding, Range, VersionedTextDocumentIdentifier};
use vun_lsp_protocol::document::{ContentChange, DidChangeParams, Severity};
use vun_lsp_protocol::rpc::ErrorCode;
use vun_lsp_protocol::{Notification, Request, notifications as n, requests as r};

use super::{Client, at, document, served};
use crate::server::Server;

#[test]
fn the_lifecycle_admits_methods_in_order() {
    let dir = Scratch::new("lsp-lifecycle");
    let uri = document(&dir, "a.vun");
    let mut session = Session::new();
    let mut server = Server::new(&mut session);
    let mut client = Client::new(&mut server);
    let early = client.request::<r::Hover>(&at(&uri, 0, 0)).unwrap_err();
    assert_eq!(early.code, ErrorCode::ServerNotInitialized);
    let init = client.initialize(&[PositionEncoding::Utf16, PositionEncoding::Utf8]);
    assert_eq!(init.capabilities.position_encoding, Some(PositionEncoding::Utf8));
    assert_eq!(init.capabilities.hover_provider, Some(true));
    assert!(init.capabilities.semantic_tokens_provider.is_some());
    let unknown = client.send(&vun_lsp_protocol::rpc::Message::request(
        vun_lsp_protocol::rpc::Id::Number(99),
        "textDocument/unknown",
        Value::Null,
    ));
    assert_eq!(unknown.and_then(|m| m.error).map(|e| e.code), Some(ErrorCode::MethodNotFound));
    let malformed = client.request::<r::Hover>(&at(&uri, 0, 0));
    assert_eq!(malformed.unwrap_err().code, ErrorCode::InvalidParams, "the document is not open");
    client.request::<r::Shutdown>(&None).unwrap();
    let late = client.request::<r::Hover>(&at(&uri, 0, 0)).unwrap_err();
    assert_eq!(late.code, ErrorCode::InvalidRequest);
    client.notify::<n::Exit>(&None);
    let _ = (<r::Hover as Request>::METHOD, <n::Exit as Notification>::METHOD);
}

#[test]
fn an_open_document_is_checked_hovered_and_followed_per_version() {
    served("lsp-session", |client, uri| {
        client.open(uri, "/// The limit.\nx: u8 = 300;\nf := () -> u8 { x };\n");
        let published = client.published();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].version, Some(1));
        let codes: Vec<_> = published[0].diagnostics.iter().filter_map(|d| d.code.as_deref()).collect();
        assert_eq!(codes, ["int.does_not_fit"]);
        assert_eq!(published[0].diagnostics[0].severity, Some(Severity::Error));

        // `300` becomes `200`: one edit, replacing the digit.
        let edit =
            Range { start: Position { line: 1, character: 8 }, end: Position { line: 1, character: 9 } };
        let change = ContentChange { range: Some(edit), text: "2".to_owned() };
        let text_document = VersionedTextDocumentIdentifier { uri: uri.clone(), version: 2 };
        client.notify::<n::DidChange>(&DidChangeParams { text_document, content_changes: vec![change] });
        let published = client.published();
        assert_eq!((published[0].version, published[0].diagnostics.len()), (Some(2), 0));

        let hover = client.request::<r::Hover>(&at(uri, 2, 16)).unwrap().unwrap();
        assert!(hover.contents.value.contains("u8") && hover.contents.value.contains("The limit."));
        let definition = client.request::<r::Definition>(&at(uri, 2, 16)).unwrap().unwrap();
        assert_eq!((&definition.uri, definition.range.start.line), (uri, 1));
    });
}
