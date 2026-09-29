//! The methods the server answers, one row each: the method's type, when in the lifecycle it is
//! answered, and the capability announcing it. A row's handler reads the method's parameters, and
//! writes its result, as the method's type says; the capabilities announced at initialization are
//! derived from the rows.

use vun_data::{ToData, Value};
use vun_grammar::TokenKind;
use vun_lsp_protocol::assist::{CompletionOptions, SignatureHelpOptions};
use vun_lsp_protocol::edit::CodeActionKind;
use vun_lsp_protocol::language::SemanticTokensLegend;
use vun_lsp_protocol::lifecycle::{
    CodeActionOptions, SemanticTokensOptions, ServerCapabilities, TextDocumentSyncKind,
    TextDocumentSyncOptions,
};
use vun_lsp_protocol::{Notification, Request, notifications as n, requests as r};

use crate::failure::Failure;
use crate::server::Server;

/// Answers the request `R`.
pub(crate) trait Answers<R: Request> {
    fn answer(&mut self, params: R::Params) -> Result<R::Result, Failure>;
}

/// Handles the notification `N`.
pub(crate) trait Hears<N: Notification> {
    fn hear(&mut self, params: N::Params) -> Result<(), Failure>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Request,
    Notification,
}

/// When in the lifecycle a method is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admitted {
    /// Before initialization and after shutdown too: the lifecycle's own methods.
    Always,
    /// Between initialization and shutdown.
    Running,
}

/// What the server announces it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Capability {
    /// Documents opened, changed by edits, and closed.
    Sync,
    Hover,
    Definition,
    Symbols,
    Formatting,
    Folding,
    SemanticTokens,
    References,
    Rename,
    CodeActions,
    SignatureHelp,
    InlayHints,
    Completion,
}

impl Capability {
    /// Announces it among `capabilities`.
    fn announce(self, capabilities: &mut ServerCapabilities) {
        match self {
            Self::Sync => {
                let sync =
                    TextDocumentSyncOptions { open_close: true, change: TextDocumentSyncKind::Incremental };
                capabilities.text_document_sync = Some(sync);
            }
            Self::Hover => capabilities.hover_provider = Some(true),
            Self::Definition => capabilities.definition_provider = Some(true),
            Self::Symbols => capabilities.document_symbol_provider = Some(true),
            Self::Formatting => capabilities.document_formatting_provider = Some(true),
            Self::Folding => capabilities.folding_range_provider = Some(true),
            Self::SemanticTokens => {
                let legend = SemanticTokensLegend::all();
                capabilities.semantic_tokens_provider =
                    Some(SemanticTokensOptions { legend, full: true, range: false });
            }
            Self::References => capabilities.references_provider = Some(true),
            Self::Rename => capabilities.rename_provider = Some(true),
            Self::CodeActions => {
                let kinds = vec![CodeActionKind::QuickFix];
                capabilities.code_action_provider = Some(CodeActionOptions { code_action_kinds: kinds });
            }
            Self::SignatureHelp => {
                let triggers = [TokenKind::LParen, TokenKind::Comma];
                let trigger_characters =
                    triggers.iter().filter_map(|t| t.text()).map(str::to_owned).collect();
                capabilities.signature_help_provider = Some(SignatureHelpOptions { trigger_characters });
            }
            Self::InlayHints => capabilities.inlay_hint_provider = Some(true),
            Self::Completion => {
                let trigger_characters = TokenKind::Dot.text().map(str::to_owned).into_iter().collect();
                capabilities.completion_provider = Some(CompletionOptions { trigger_characters });
            }
        }
    }
}

/// How a method is handled: its parameters read, its result written, `null` for a notification.
pub(crate) type Handle = for<'s> fn(&mut Server<'s>, &Value<'_>) -> Result<Value<'static>, Failure>;

pub(crate) struct Method {
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    pub(crate) admitted: Admitted,
    pub(crate) handle: Handle,
    pub(crate) capability: Option<Capability>,
}

fn answer<R: Request>(server: &mut Server<'_>, params: &Value<'_>) -> Result<Value<'static>, Failure>
where
    for<'s> Server<'s>: Answers<R>,
{
    let params = params.read::<R::Params>().map_err(Failure::params)?;
    Answers::<R>::answer(server, params).map(|result| result.to_data())
}

fn hear<N: Notification>(server: &mut Server<'_>, params: &Value<'_>) -> Result<Value<'static>, Failure>
where
    for<'s> Server<'s>: Hears<N>,
{
    let params = params.read::<N::Params>().map_err(Failure::params)?;
    Hears::<N>::hear(server, params).map(|()| Value::Null)
}

const fn request<R: Request>(admitted: Admitted, capability: Option<Capability>) -> Method
where
    for<'s> Server<'s>: Answers<R>,
{
    Method { name: R::METHOD, kind: Kind::Request, admitted, handle: answer::<R>, capability }
}

const fn notification<N: Notification>(admitted: Admitted, capability: Option<Capability>) -> Method
where
    for<'s> Server<'s>: Hears<N>,
{
    Method { name: N::METHOD, kind: Kind::Notification, admitted, handle: hear::<N>, capability }
}

/// A method of the running server, announced by `capability`.
const fn feature<R: Request>(capability: Capability) -> Method
where
    for<'s> Server<'s>: Answers<R>,
{
    request::<R>(Admitted::Running, Some(capability))
}

pub(crate) const METHODS: &[Method] = &[
    request::<r::Initialize>(Admitted::Always, None),
    notification::<n::Initialized>(Admitted::Running, None),
    request::<r::Shutdown>(Admitted::Running, None),
    notification::<n::Exit>(Admitted::Always, None),
    notification::<n::DidOpen>(Admitted::Running, Some(Capability::Sync)),
    notification::<n::DidChange>(Admitted::Running, Some(Capability::Sync)),
    notification::<n::DidClose>(Admitted::Running, Some(Capability::Sync)),
    feature::<r::Hover>(Capability::Hover),
    feature::<r::Definition>(Capability::Definition),
    feature::<r::DocumentSymbol>(Capability::Symbols),
    feature::<r::Formatting>(Capability::Formatting),
    feature::<r::FoldingRange>(Capability::Folding),
    feature::<r::SemanticTokens>(Capability::SemanticTokens),
    feature::<r::References>(Capability::References),
    feature::<r::Rename>(Capability::Rename),
    feature::<r::CodeAction>(Capability::CodeActions),
    feature::<r::SignatureHelp>(Capability::SignatureHelp),
    feature::<r::InlayHint>(Capability::InlayHints),
    feature::<r::Completion>(Capability::Completion),
];

impl Method {
    /// The row of the method `name`.
    pub(crate) fn named(name: &str) -> Option<&'static Self> {
        METHODS.iter().find(|m| m.name == name)
    }

    /// What the rows announce.
    pub(crate) fn capabilities() -> ServerCapabilities {
        let mut capabilities = ServerCapabilities::default();
        for capability in METHODS.iter().filter_map(|m| m.capability) {
            capability.announce(&mut capabilities);
        }
        capabilities
    }
}
