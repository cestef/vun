//! The rules the grammar gives: keywords, contextual words, names placed by the tokens around
//! them, and punctuation.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::syntax::{Rule, Scope};
use super::{Recognized, Scoped, lexical};
use crate::grammar::lexical::{IDENT, TYPE_NAME};
use crate::grammar::model::Token;
use crate::grammar::pattern::Pattern;

impl Scoped<'_> {
    /// The spelling of `token`, as a pattern.
    fn token(&self, token: Token) -> Pattern {
        Pattern::literal(self.model.text(token).unwrap_or_default())
    }

    fn texts(&self, tokens: &[Token]) -> Vec<&str> {
        tokens.iter().filter_map(|&t| self.model.text(t)).collect()
    }

    pub(super) fn keywords(&self) -> Vec<Rule> {
        let sublime = &self.editors.sublime;
        let keywords = self.texts(&sublime.keywords);
        let borrow = self.token(sublime.borrow);
        let mut after_borrow = vec![borrow.clone()];
        after_borrow.extend(
            sublime
                .qualifiers
                .iter()
                .map(|q| borrow.clone().then(Pattern::literal(q)).then(Pattern::raw(r"\s"))),
        );
        let words = sublime
            .qualifiers
            .iter()
            .chain(&sublime.modes)
            .map(String::as_str)
            .filter(|w| !keywords.contains(w))
            .collect::<Vec<_>>();
        let mut rules = vec![Rule::of(Pattern::words(words).after(&after_borrow), Scope::Borrow)];
        rules.extend(self.contextual());
        rules.push(Rule::of(Pattern::words(keywords), Scope::Keyword));
        rules
    }

    /// A rule per contextual keyword, placed as its context says.
    fn contextual(&self) -> Vec<Rule> {
        let rules = self.model.contextual.iter().map(|(text, recognized)| {
            let word = Pattern::words([text.as_str()]);
            match recognized {
                Recognized::ForPattern => Rule::of(word, Scope::Contextual),
            }
        });
        rules.collect()
    }

    pub(super) fn names(&self) -> Vec<Rule> {
        let sublime = &self.editors.sublime;
        let mut closing = Vec::new();
        for &(_, close) in &self.model.delimiters {
            let closer = self.token(close);
            if !closing.contains(&closer) {
                closing.push(closer);
            }
        }
        // An inferred member, `.ok`, not a path's segment: no name or closed group before it.
        let member = self
            .token(sublime.member)
            .then(Pattern::raw(IDENT))
            .not_after(&Pattern::any(closing.into_iter().chain([Pattern::raw(r"\w")])));
        let mut rules = lexical::names();
        rules.extend([
            Rule::of(Pattern::raw(TYPE_NAME).word(), Scope::Type),
            Rule::of(
                Pattern::raw(IDENT).word().before(&Pattern::space().then(self.token(sublime.binds))),
                Scope::Definition,
            ),
            Rule::of(member, Scope::Member),
            Rule::of(
                Pattern::raw(IDENT).word().before(&Pattern::space().then(self.token(sublime.call))),
                Scope::Call,
            ),
        ]);
        rules
    }

    /// Each punctuation token scoped as said apart or by the first table it is in, longest
    /// tokens first so that `==` is never read as `=` twice.
    pub(super) fn punctuation(&self) -> Vec<Rule> {
        let sublime = &self.editors.sublime;
        let mut groups: BTreeMap<(Reverse<usize>, Scope), Vec<&str>> = BTreeMap::new();
        for &token in &sublime.punctuation {
            let Some(text) = self.model.text(token) else { continue };
            let special = sublime.special.iter().find(|(t, _)| *t == token).map(|&(_, scope)| scope);
            let listed =
                || sublime.tables.iter().find(|(tokens, _)| tokens.contains(&token)).map(|&(_, scope)| scope);
            let scope = special.or_else(listed).unwrap_or(Scope::Punctuation);
            groups.entry((Reverse(text.len()), scope)).or_default().push(text);
        }
        groups.into_iter().map(|((_, scope), texts)| Rule::of(Self::spelled(&texts), scope)).collect()
    }

    /// Any of `texts`, a word-like one only as a whole word: `_` but not the one in `a_b`.
    fn spelled(texts: &[&str]) -> Pattern {
        Pattern::any(texts.iter().map(|text| {
            let literal = Pattern::literal(text);
            if text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { literal.word() } else { literal }
        }))
    }
}
