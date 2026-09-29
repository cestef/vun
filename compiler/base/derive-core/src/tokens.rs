//! Building token streams tree by tree, as the generated code is never text.

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// Tokens appended in order.
#[derive(Clone, Default)]
#[must_use]
pub struct Tokens(Vec<TokenTree>);

impl Tokens {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tree(mut self, tree: impl Into<TokenTree>) -> Self {
        self.0.push(tree.into());
        self
    }

    pub fn all(mut self, trees: impl IntoIterator<Item = TokenTree>) -> Self {
        self.0.extend(trees);
        self
    }

    pub fn word(self, word: &str) -> Self {
        self.tree(Ident::new(word, Span::call_site()))
    }

    /// A punctuation of one or more characters, such as `::` or `=>`.
    pub fn op(mut self, op: &str) -> Self {
        let mut chars = op.chars().peekable();
        while let Some(c) = chars.next() {
            let spacing = if chars.peek().is_some() { Spacing::Joint } else { Spacing::Alone };
            self.0.push(Punct::new(c, spacing).into());
        }
        self
    }

    pub fn lifetime(self, name: &str) -> Self {
        self.tree(Punct::new('\'', Spacing::Joint)).word(name)
    }

    /// An absolute path, as `::vun_data::ToData`.
    pub fn path(self, segments: &[&str]) -> Self {
        segments.iter().fold(self, |tokens, segment| tokens.op("::").word(segment))
    }

    pub fn number(self, n: u32) -> Self {
        self.tree(Literal::u32_suffixed(n))
    }

    /// An integer literal typed by where it is used.
    pub fn count(self, n: usize) -> Self {
        self.tree(Literal::usize_unsuffixed(n))
    }

    /// A string literal.
    pub fn string(self, text: &str) -> Self {
        self.tree(Literal::string(text))
    }

    pub fn group(self, delimiter: Delimiter, inner: Self) -> Self {
        self.tree(Group::new(delimiter, inner.into()))
    }

    pub fn parens(self, inner: Self) -> Self {
        self.group(Delimiter::Parenthesis, inner)
    }

    pub fn braces(self, inner: Self) -> Self {
        self.group(Delimiter::Brace, inner)
    }

    /// An outer attribute `#[inner]`.
    pub fn attr(self, inner: Self) -> Self {
        self.op("#").group(Delimiter::Bracket, inner)
    }

    /// Each of `items` built by `each`, separated by commas.
    pub fn comma<T>(self, items: impl IntoIterator<Item = T>, each: impl Fn(Self, T) -> Self) -> Self {
        items.into_iter().fold(self, |tokens, item| each(tokens, item).op(","))
    }

    pub fn then(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    /// `<a, b>`, or nothing for no items.
    pub fn angled(self, items: impl ExactSizeIterator<Item = Self>) -> Self {
        if items.len() == 0 {
            return self;
        }
        self.op("<").comma(items, Self::then).op(">")
    }

    /// `::core::result::Result::Ok(value)`
    pub fn ok(self, value: Self) -> Self {
        self.path(&["core", "result", "Result", "Ok"]).parens(value)
    }

    /// `::core::result::Result::Err(value)`
    pub fn err(self, value: Self) -> Self {
        self.path(&["core", "result", "Result", "Err"]).parens(value)
    }

    /// `::core::result::Result<Self, error>`
    pub fn result(self, error: Self) -> Self {
        self.path(&["core", "result", "Result"]).op("<").word("Self").op(",").then(error).op(">")
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Tokens> for TokenStream {
    fn from(tokens: Tokens) -> Self {
        tokens.0.into_iter().collect()
    }
}
