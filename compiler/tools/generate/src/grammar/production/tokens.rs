//! Resolving the tokens of productions: token names, lists of the token kind and tables of tokens
//! to token kinds, spellings to tokens or words, word kinds to their spellings.

use std::borrow::Cow;

use vun_diag::Msg;
use vun_syntax::literal::{Decl, Literal};

use super::{Resolved, Term, Written};
use crate::generate::Read;
use crate::grammar::{Enum, GENERATOR, Grammar, SHAPE, UNRESOLVED, Variant};
use crate::names::Name;

/// How the terms of a kind's productions resolve: against the grammar, its declarations and the
/// kind of tokens, the one kind with every token name the productions write.
pub(in crate::grammar) struct Tokens<'g, 's> {
    grammar: &'g Grammar<'s>,
    decls: &'g [Decl<'s>],
    pub(in crate::grammar) kind: &'g Enum<'s>,
}

impl<'g, 's> Tokens<'g, 's> {
    pub(in crate::grammar) fn of(
        grammar: &'g Grammar<'s>,
        decls: &'g [Decl<'s>],
        of: &Enum<'s>,
    ) -> Read<Self> {
        let mut leaves = Vec::new();
        for v in &of.variants {
            for t in v.production.iter().flatten() {
                t.leaves(&mut leaves);
            }
        }
        let written = leaves.into_iter().filter_map(|l| match l {
            Written::Token(names) => Some(names),
            Written::Text(_) | Written::Word(_) => None,
        });
        let names: Vec<&str> =
            written.flatten().copied().filter(|n| Self::expanded(grammar, decls, n).is_none()).collect();
        let kind = &grammar.enums[grammar.resolve(&of.name, &names)?];
        Ok(Self { grammar, decls, kind })
    }

    /// The token names a list of the kind or a table stands for, or `None` for another name.
    fn expanded(grammar: &Grammar<'s>, decls: &[Decl<'s>], name: &str) -> Option<Vec<&'s str>> {
        if let Some(table) = decls.iter().find(|d| d.name == name && !d.name.is_type_name()) {
            let mut names = Vec::new();
            Self::mentioned(&table.value, &mut names);
            return Some(names);
        }
        let mut lists = grammar.enums.iter().flat_map(|e| &e.lists);
        lists.find(|(list, _)| *list == name).map(|(_, names)| names.clone())
    }

    /// Every `.name` a value holds, at any depth.
    fn mentioned(value: &Literal<'s>, into: &mut Vec<&'s str>) {
        match value {
            Literal::Name(name) => into.push(name),
            Literal::List(items) => items.iter().for_each(|v| Self::mentioned(v, into)),
            Literal::Record(fields) => fields.iter().for_each(|f| Self::mentioned(&f.value, into)),
            _ => {}
        }
    }

    /// The token kinds a table of tokens or a list of the token kind names.
    pub(in crate::grammar) fn names(&self, table: &str) -> Option<Vec<&'s str>> {
        let names = Self::expanded(self.grammar, self.decls, table)?;
        Some(names.into_iter().filter(|n| self.has(n)).collect())
    }

    /// The spellings of the tokens a table of tokens or a list of the token kind names.
    pub(in crate::grammar) fn spellings(&self, name: &str) -> Option<Vec<&'s str>> {
        let names = Self::expanded(self.grammar, self.decls, name)?;
        let texts = self.kind.variants.iter().filter(|v| names.contains(&v.name));
        Some(
            texts
                .filter_map(|v| match &v.text {
                    Some(Cow::Borrowed(text)) => Some(*text),
                    _ => None,
                })
                .collect(),
        )
    }

    /// The terms of the production of `owner`, their tokens resolved.
    pub(in crate::grammar) fn resolve(
        &self,
        owner: &str,
        terms: &[Term<'s, Written<'s>>],
    ) -> Read<Vec<Term<'s, Resolved<'s>>>> {
        terms.iter().map(|t| t.map(&mut |leaf| self.leaf(owner, leaf))).collect()
    }

    fn leaf(&self, owner: &str, leaf: &Written<'s>) -> Read<Resolved<'s>> {
        Ok(match leaf {
            Written::Text(text) => {
                match self.kind.variants.iter().find(|v| v.text.as_deref() == Some(text)) {
                    Some(v) => Resolved::Tokens(vec![v.name]),
                    None if text.chars().all(|c| c.is_alphanumeric() || c == '_') => {
                        Resolved::Words(vec![text.clone()])
                    }
                    None => {
                        let msg = Msg::new().text("no token is spelled ").code(text.as_ref());
                        return Err(GENERATOR.error(UNRESOLVED, owner, msg));
                    }
                }
            }
            Written::Token(names) => {
                let mut kinds = Vec::new();
                for name in names {
                    match Self::expanded(self.grammar, self.decls, name) {
                        Some(listed) => kinds.extend(listed.into_iter().filter(|n| self.has(n))),
                        None => kinds.push(*name),
                    }
                }
                Resolved::Tokens(kinds)
            }
            Written::Word(words) => {
                let variants = self.words(words).ok_or_else(|| {
                    let msg = Msg::new().text("no kind or list of one is named ").code(*words);
                    GENERATOR.error(UNRESOLVED, owner, msg)
                })?;
                let texts = variants.iter().map(|v| v.text.clone()).collect::<Option<Vec<_>>>();
                Resolved::Words(texts.ok_or_else(|| {
                    GENERATOR.error(SHAPE, owner, Msg::new().code(*words).text(" has an unspelled variant"))
                })?)
            }
        })
    }

    /// The variants a kind, or a list of a kind, named `name` holds.
    fn words(&self, name: &str) -> Option<Vec<&'g Variant<'s>>> {
        let enums = &self.grammar.enums;
        if let Some(kind) = enums.iter().find(|e| e.name == name) {
            return Some(kind.variants.iter().collect());
        }
        enums.iter().find_map(|e| {
            let (_, names) = e.lists.iter().find(|(list, _)| *list == name)?;
            Some(e.variants.iter().filter(|v| names.contains(&v.name)).collect())
        })
    }

    fn has(&self, name: &str) -> bool {
        self.kind.variants.iter().any(|v| v.name == name)
    }
}
