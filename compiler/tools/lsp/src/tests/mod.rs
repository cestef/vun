mod assist;
mod command;
mod edit;
mod features;
mod session;
mod wire;

use vun_data::{ToData, Value};
use vun_driver::Session;
use vun_fs::Scratch;
use vun_lsp_protocol::base::{
    Position, PositionEncoding, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
};
use vun_lsp_protocol::document::{DidOpenParams, PublishDiagnosticsParams};
use vun_lsp_protocol::lifecycle::{
    ClientCapabilities, GeneralCapabilities, InitializeParams, InitializeResult, Offered,
};
use vun_lsp_protocol::rpc::{Id, Message, ResponseError};
use vun_lsp_protocol::{Notification, Request, notifications as n, requests as r};

use crate::server::Server;

/// A client of `server`: each message it sends, and what the server answered or published.
struct Client<'s, 'd> {
    server: &'s mut Server<'d>,
    next: i64,
}

impl<'s, 'd> Client<'s, 'd> {
    fn new(server: &'s mut Server<'d>) -> Self {
        Self { server, next: 0 }
    }

    /// Sends `message` as text, and reads what the server answers.
    fn send(&mut self, message: &Message) -> Option<Message> {
        let reply = self.server.handle(&vun_data::json::Json(&message.to_data()).to_string())?;
        Some(reply.to_data().read().expect("the server writes messages"))
    }

    fn request<R: Request>(&mut self, params: &R::Params) -> Result<R::Result, ResponseError> {
        self.next += 1;
        let message = Message::request(Id::Number(self.next), R::METHOD, params.to_data());
        let reply = self.send(&message).expect("a request is answered");
        assert_eq!(reply.id, Some(Id::Number(self.next)));
        // A response without an error succeeds; `null` results read as absent.
        match reply.error {
            Some(error) => Err(error),
            None => Ok(reply.result.unwrap_or_default().read().expect("the result fits the method")),
        }
    }

    fn notify<N: Notification>(&mut self, params: &N::Params) {
        assert!(self.send(&Message::notification(N::METHOD, params.to_data())).is_none());
    }

    /// The diagnostics published since the last call, by document.
    fn published(&mut self) -> Vec<PublishDiagnosticsParams> {
        self.server.publish();
        let messages = self.server.outbox.drain(..);
        messages.map(|m| m.params.unwrap_or_default().read().expect("publishes diagnostics")).collect()
    }

    fn initialize(&mut self, encodings: &[PositionEncoding]) -> InitializeResult {
        let offered = encodings.iter().map(|&e| Offered::Known(e)).collect();
        let general = GeneralCapabilities { position_encodings: Some(offered) };
        let capabilities = ClientCapabilities { general: Some(general) };
        let result =
            self.request::<r::Initialize>(&InitializeParams { capabilities, ..InitializeParams::default() });
        self.notify::<n::Initialized>(&Value::Null);
        result.unwrap()
    }

    fn open(&mut self, uri: &Uri, text: &str) {
        let text_document = TextDocumentItem {
            uri: uri.clone(),
            language_id: "vun".to_owned(),
            version: 1,
            text: text.to_owned(),
        };
        self.notify::<n::DidOpen>(&DidOpenParams { text_document });
    }
}

/// A file of a scratch directory, and its URI.
fn document(dir: &Scratch, name: &str) -> Uri {
    let path = std::path::absolute(dir.write(name, "")).unwrap();
    Uri::from_file_path(&path).unwrap()
}

fn at(uri: &Uri, line: u32, character: u32) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position: Position { line, character },
    }
}

/// Runs `test` against a server of a new session, initialized with UTF-8 positions.
fn served(name: &str, test: impl FnOnce(&mut Client<'_, '_>, &Uri)) {
    let dir = Scratch::new(name);
    let uri = document(&dir, "a.vun");
    let mut session = Session::new();
    session.registry.formatters.link(Box::new(vun_fmt::Vun));
    let mut server = Server::new(&mut session);
    let mut client = Client::new(&mut server);
    client.initialize(&[PositionEncoding::Utf8]);
    test(&mut client, &uri);
}
