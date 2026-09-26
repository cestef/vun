//! Building token streams tree by tree, as the generated code is never text.

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// Tokens appended in order.
#[derive(Clone, Default)]
pub(crate) struct Tokens(Vec<TokenTree>);

impl Tokens {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn tree(mut self, tree: impl Into<TokenTree>) -> Self {
        self.0.push(tree.into());
        self
    }

    pub(crate) fn all(mut self, trees: impl IntoIterator<Item = TokenTree>) -> Self {
        self.0.extend(trees);
        self
    }

    pub(crate) fn word(self, word: &str) -> Self {
        self.tree(Ident::new(word, Span::call_site()))
    }

    /// A punctuation of one or more characters, such as `::` or `=>`.
    pub(crate) fn op(mut self, op: &str) -> Self {
        let mut chars = op.chars().peekable();
        while let Some(c) = chars.next() {
            let spacing = if chars.peek().is_some() { Spacing::Joint } else { Spacing::Alone };
            self.0.push(Punct::new(c, spacing).into());
        }
        self
    }

    pub(crate) fn lifetime(self, name: &str) -> Self {
        self.tree(Punct::new('\'', Spacing::Joint)).word(name)
    }

    /// An absolute path, as `::vun_codec::Encode`.
    pub(crate) fn path(self, segments: &[&str]) -> Self {
        segments.iter().fold(self, |tokens, segment| tokens.op("::").word(segment))
    }

    /// A path in `vun_codec`.
    pub(crate) fn codec(self, name: &str) -> Self {
        self.path(&["vun_codec", name])
    }

    pub(crate) fn number(self, n: u32) -> Self {
        self.tree(Literal::u32_suffixed(n))
    }

    pub(crate) fn group(self, delimiter: Delimiter, inner: Self) -> Self {
        self.tree(Group::new(delimiter, inner.into()))
    }

    pub(crate) fn parens(self, inner: Self) -> Self {
        self.group(Delimiter::Parenthesis, inner)
    }

    pub(crate) fn braces(self, inner: Self) -> Self {
        self.group(Delimiter::Brace, inner)
    }

    /// An outer attribute `#[inner]`.
    pub(crate) fn attr(self, inner: Self) -> Self {
        self.op("#").group(Delimiter::Bracket, inner)
    }

    /// Each of `items` built by `each`, separated by commas.
    pub(crate) fn comma<T>(self, items: impl IntoIterator<Item = T>, each: impl Fn(Self, T) -> Self) -> Self {
        items.into_iter().fold(self, |tokens, item| each(tokens, item).op(","))
    }

    pub(crate) fn then(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    /// `<a, b>`, or nothing for no items.
    pub(crate) fn angled(self, items: impl ExactSizeIterator<Item = Self>) -> Self {
        if items.len() == 0 {
            return self;
        }
        self.op("<").comma(items, Self::then).op(">")
    }

    /// `::vun_codec::Encode::encode(value, w);`
    pub(crate) fn encoding(self, value: Self) -> Self {
        self.codec("Encode").op("::").word("encode").parens(value.op(",").word("w")).op(";")
    }

    /// `::vun_codec::Decode::decode(r)?`
    pub(crate) fn decoding(self) -> Self {
        self.codec("Decode").op("::").word("decode").parens(Self::new().word("r")).op("?")
    }

    /// `::core::result::Result::Ok(value)`
    pub(crate) fn ok(self, value: Self) -> Self {
        self.path(&["core", "result", "Result", "Ok"]).parens(value)
    }

    /// `::vun_codec::Writer<'_>`
    pub(crate) fn writer(self) -> Self {
        self.codec("Writer").op("<").lifetime("_").op(">")
    }

    /// `::vun_codec::Reader<'_>`
    pub(crate) fn reader(self) -> Self {
        self.codec("Reader").op("<").lifetime("_").op(">")
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Tokens> for TokenStream {
    fn from(tokens: Tokens) -> Self {
        tokens.0.into_iter().collect()
    }
}
