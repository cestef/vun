//! The protocol's methods as types: each a name, the parameters it takes and, for a request, the
//! result it answers with. A method is one row of the tables below.

use vun_json::{FromJson, ToJson, Value};

use crate::{base, document, language, lifecycle};

/// A method answered by a response.
pub trait Request {
    const METHOD: &'static str;
    type Params: FromJson + ToJson;
    type Result: FromJson + ToJson;
}

/// A method answered by nothing.
pub trait Notification {
    const METHOD: &'static str;
    type Params: FromJson + ToJson;
}

macro_rules! methods {
    ($kind:ident { $($(#[$doc:meta])* $name:ident = $method:literal: $params:ty $(=> $result:ty)?;)* }) => {$(
        $(#[$doc])*
        #[derive(Debug)]
        pub enum $name {}

        impl $kind for $name {
            const METHOD: &'static str = $method;
            type Params = $params;
            $(type Result = $result;)?
        }
    )*};
}

/// The requests, by the types naming them.
pub mod requests {
    use super::{Request, Value, base, language, lifecycle};

    methods!(Request {
        Initialize = "initialize": lifecycle::InitializeParams => lifecycle::InitializeResult;
        Shutdown = "shutdown": Option<Value<'static>> => ();
        Hover = "textDocument/hover": base::TextDocumentPositionParams => Option<language::Hover>;
        Definition = "textDocument/definition": base::TextDocumentPositionParams => Option<base::Location>;
        DocumentSymbol = "textDocument/documentSymbol": base::TextDocumentParams => Vec<language::DocumentSymbol>;
        SemanticTokens = "textDocument/semanticTokens/full": base::TextDocumentParams => language::SemanticTokens;
        Formatting = "textDocument/formatting": language::FormattingParams => Option<Vec<base::TextEdit>>;
        FoldingRange = "textDocument/foldingRange": base::TextDocumentParams => Vec<language::FoldingRange>;
    });
}

/// The notifications, by the types naming them.
pub mod notifications {
    use super::{Notification, Value, document};

    methods!(Notification {
        Initialized = "initialized": Value<'static>;
        Exit = "exit": Option<Value<'static>>;
        DidOpen = "textDocument/didOpen": document::DidOpenParams;
        DidChange = "textDocument/didChange": document::DidChangeParams;
        DidClose = "textDocument/didClose": document::DidCloseParams;
        PublishDiagnostics = "textDocument/publishDiagnostics": document::PublishDiagnosticsParams;
    });
}
