//! The rules the grammar's tables give: keywords, contextual words, names placed by the tokens
//! around them, and punctuation.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use vun_syntax::literal::Literal;

use super::syntax::{Rule, Scope};
use super::{
    BINDS, BLOCK, BORROW, BORROWS, CALL, CONTEXTUAL, DELIMITERS, DOT, FLOWS, IDENT, KEYWORDS, MODES,
    PUNCTUATION, QUALIFIERS, Recognized, SPECIAL, TABLES, TOKENS, TYPE_NAME, Tables, lexical,
};
use crate::generate::Read;
use crate::grammar::pattern::Pattern;

/// Fields of a row of `contextual`.
const TEXT: &str = "text";
const CONTEXT: &str = "context";
/// The field of a row of `delimiters` naming its closer.
const CLOSE: &str = "close";

impl Tables<'_, '_> {
    pub(super) fn keywords(&self) -> Read<Vec<Rule>> {
        let keywords = self.texts(TOKENS, KEYWORDS)?;
        let block = self.token(BLOCK)?;
        let flows = Pattern::words(self.texts(FLOWS.0, FLOWS.1)?)
            .after(&[block.clone(), block.clone().then(Pattern::raw(r"\s"))]);
        let qualifiers = self.texts(BORROWS, QUALIFIERS)?;
        let borrow = self.token(BORROW)?;
        let mut after_borrow = vec![borrow.clone()];
        after_borrow.extend(
            qualifiers.iter().map(|q| borrow.clone().then(Pattern::literal(q)).then(Pattern::raw(r"\s"))),
        );
        let words = qualifiers
            .iter()
            .chain(&self.texts(BORROWS, MODES)?)
            .copied()
            .filter(|w| !keywords.contains(w))
            .collect::<Vec<_>>();
        let mut rules = vec![
            Rule::of(flows, Scope::Flow),
            Rule::of(Pattern::words(words).after(&after_borrow), Scope::Borrow),
        ];
        rules.extend(self.contextual()?);
        rules.push(Rule::of(Pattern::words(keywords), Scope::Keyword));
        Ok(rules)
    }

    /// A rule per contextual keyword, placed as its context says.
    fn contextual(&self) -> Read<Vec<Rule>> {
        let rows = self.table(CONTEXTUAL)?.as_list().ok_or_else(|| Self::missing(CONTEXTUAL))?;
        rows.iter()
            .map(|row| {
                let (Some(Literal::String(text)), Some(Literal::Name(context))) =
                    (row.get(TEXT), row.get(CONTEXT))
                else {
                    return Err(Self::missing(CONTEXTUAL));
                };
                let (_, recognized) = Recognized::ALL
                    .iter()
                    .find(|(name, _)| name == context)
                    .ok_or_else(|| Self::missing(context))?;
                let word = Pattern::words([text.as_ref()]);
                Ok(match recognized {
                    Recognized::ForPattern => Rule::of(word, Scope::Contextual),
                    // `.asm {`: after the dot, which stays punctuation.
                    Recognized::AsmBlock => Rule::of(
                        word.after(&[self.token(DOT)?]).before(&Pattern::space().then(self.token(BLOCK)?)),
                        Scope::Contextual,
                    ),
                })
            })
            .collect()
    }

    pub(super) fn names(&self) -> Read<Vec<Rule>> {
        let closers = self.table(DELIMITERS)?.as_list().ok_or_else(|| Self::missing(DELIMITERS))?;
        let mut closing = Vec::new();
        for name in closers.iter().filter_map(|row| row.get(CLOSE)?.as_name()) {
            let closer = self.token(name)?;
            if !closing.contains(&closer) {
                closing.push(closer);
            }
        }
        // An inferred member, `.ok`, not a path's segment: no name or closed group before it.
        let member = self
            .token(DOT)?
            .then(Pattern::raw(IDENT))
            .not_after(&Pattern::any(closing.into_iter().chain([Pattern::raw(r"\w")])));
        let mut rules = lexical::names();
        rules.extend([
            Rule::of(Pattern::raw(TYPE_NAME).word(), Scope::Type),
            Rule::of(
                Pattern::raw(IDENT).word().before(&Pattern::space().then(self.token(BINDS)?)),
                Scope::Definition,
            ),
            Rule::of(member, Scope::Member),
            Rule::of(
                Pattern::raw(IDENT).word().before(&Pattern::space().then(self.token(CALL)?)),
                Scope::Call,
            ),
        ]);
        Ok(rules)
    }

    /// Each punctuation token scoped by the first of `SPECIAL` or `TABLES` it is in, longest
    /// tokens first so that `==` is never read as `=` twice.
    pub(super) fn punctuation(&self) -> Read<Vec<Rule>> {
        let tokens = self.kind(TOKENS)?;
        let (_, names) =
            tokens.lists.iter().find(|(l, _)| *l == PUNCTUATION).ok_or_else(|| Self::missing(PUNCTUATION))?;
        let mut tables = Vec::new();
        for (table, scope) in TABLES {
            let mut mentioned = Vec::new();
            Self::mentioned(self.table(table)?, &mut mentioned);
            tables.push((mentioned, scope));
        }
        let mut groups: BTreeMap<(Reverse<usize>, Scope), Vec<&str>> = BTreeMap::new();
        for &name in names {
            let text = tokens.text(name)?;
            let special = SPECIAL.iter().find(|(n, _)| *n == name).map(|&(_, scope)| scope);
            let listed = || tables.iter().find(|(names, _)| names.contains(&name)).map(|&(_, scope)| scope);
            let scope = special.or_else(listed).unwrap_or(Scope::Punctuation);
            groups.entry((Reverse(text.len()), scope)).or_default().push(text);
        }
        Ok(groups.into_iter().map(|((_, scope), texts)| Rule::of(Self::spelled(&texts), scope)).collect())
    }

    /// Any of `texts`, a word-like one only as a whole word: `_` but not the one in `a_b`.
    fn spelled(texts: &[&str]) -> Pattern {
        Pattern::any(texts.iter().map(|text| {
            let literal = Pattern::literal(text);
            if text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { literal.word() } else { literal }
        }))
    }
}
