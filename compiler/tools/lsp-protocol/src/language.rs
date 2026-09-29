//! What the server tells of a document: hovers, symbols, semantic tokens, formatting edits and
//! folding ranges.

use vun_json::{FromJson, ToJson};
use vun_variants::Variants;

use crate::base::{MarkupContent, Range, TextDocumentIdentifier};

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
pub struct Hover {
    pub contents: MarkupContent,
    pub range: Option<Range>,
}

/// A declaration of a document, with those it holds.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    /// The whole declaration.
    pub range: Range,
    /// Its name.
    pub selection_range: Range,
    pub children: Option<Vec<DocumentSymbol>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(number)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

/// The kinds and modifiers semantic tokens are numbered by, announced at initialization.
#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct SemanticTokensLegend {
    pub token_types: Vec<TokenType>,
    pub token_modifiers: Vec<TokenModifier>,
}

impl SemanticTokensLegend {
    /// Every kind and modifier, each numbered by its position.
    #[must_use]
    pub fn all() -> Self {
        Self { token_types: TokenType::ALL.to_vec(), token_modifiers: TokenModifier::ALL.to_vec() }
    }
}

/// Semantic tokens, five numbers each: the line from the previous token's, the start from the
/// previous token's on the same line or from the line's start, the length, the kind and the set
/// of modifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct SemanticTokens {
    pub result_id: Option<String>,
    pub data: Vec<u32>,
}

/// What a token is, by the protocol's names; its number is its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson, Variants)]
#[json(case = camel)]
pub enum TokenType {
    Namespace,
    Type,
    Struct,
    Enum,
    EnumMember,
    Interface,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    Function,
    Method,
    Macro,
    Keyword,
    Modifier,
    Comment,
    String,
    Number,
    Operator,
    Label,
    Decorator,
}

impl TokenType {
    /// Its number in the legend.
    #[must_use]
    pub fn number(self) -> u32 {
        self as u32
    }
}

/// What a token is besides its kind; its bit is its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson, Variants)]
#[json(case = camel)]
pub enum TokenModifier {
    Declaration,
    Readonly,
    Documentation,
    DefaultLibrary,
    /// Mutable: declared `mut`.
    Mutable,
    /// Known at compile time: under `$`.
    Comptime,
    Unsafe,
}

impl TokenModifier {
    /// Its bit in a token's set of modifiers.
    #[must_use]
    pub fn bit(self) -> u32 {
        1 << self as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct FormattingParams {
    pub text_document: TextDocumentIdentifier,
    /// The client's preferences, which the formatter has none of.
    pub options: vun_json::Value<'static>,
}

#[derive(Clone, Debug, PartialEq, Eq, ToJson, FromJson)]
#[json(case = camel)]
pub struct FoldingRange {
    pub start_line: u32,
    pub start_character: Option<u32>,
    pub end_line: u32,
    pub end_character: Option<u32>,
    pub kind: Option<FoldingRangeKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ToJson, FromJson)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}
