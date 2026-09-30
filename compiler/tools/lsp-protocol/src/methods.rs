//! The protocol's methods as types: each a name, the parameters it takes and, for a request, the
//! result it answers with. A method is one row of the tables below.

use vun_data::{FromData, ToData, Value};

use crate::{assist, base, command, document, edit, language, lifecycle};

/// A method answered by a response.
pub trait Request {
    const METHOD: &'static str;
    type Params: FromData + ToData;
    type Result: FromData + ToData;
}

/// A method answered by nothing.
pub trait Notification {
    const METHOD: &'static str;
    type Params: FromData + ToData;
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
    use super::{Request, Value, assist, base, command, edit, language, lifecycle};

    methods!(Request {
        Initialize = "initialize": lifecycle::InitializeParams => lifecycle::InitializeResult;
        Shutdown = "shutdown": Option<Value<'static>> => ();
        Hover = "textDocument/hover": base::TextDocumentPositionParams => Option<language::Hover>;
        Definition = "textDocument/definition": base::TextDocumentPositionParams => Option<base::Location>;
        DocumentSymbol = "textDocument/documentSymbol": base::TextDocumentParams => Vec<language::DocumentSymbol>;
        SemanticTokens = "textDocument/semanticTokens/full": base::TextDocumentParams => language::SemanticTokens;
        Formatting = "textDocument/formatting": language::FormattingParams => Option<Vec<base::TextEdit>>;
        FoldingRange = "textDocument/foldingRange": base::TextDocumentParams => Vec<language::FoldingRange>;
        References = "textDocument/references": edit::ReferenceParams => Vec<base::Location>;
        Rename = "textDocument/rename": edit::RenameParams => Option<edit::WorkspaceEdit>;
        CodeAction = "textDocument/codeAction": edit::CodeActionParams => Vec<edit::CodeAction>;
        SignatureHelp = "textDocument/signatureHelp": base::TextDocumentPositionParams => Option<assist::SignatureHelp>;
        InlayHint = "textDocument/inlayHint": assist::InlayHintParams => Option<Vec<assist::InlayHint>>;
        Completion = "textDocument/completion": base::TextDocumentPositionParams => Vec<assist::CompletionItem>;
        CodeLens = "textDocument/codeLens": command::CodeLensParams => Vec<command::CodeLens>;
        ExecuteCommand = "workspace/executeCommand": command::ExecuteCommandParams => Option<Value<'static>>;
        /// Sent by the server.
        ShowDocument = "window/showDocument": command::ShowDocumentParams => command::ShowDocumentResult;
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
