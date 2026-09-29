//! Reading the model: the files read by type, then every name resolved against the kinds of
//! tokens and nodes and every path against the grammar's files.

use std::collections::BTreeMap;

use vun_diag::Msg;
use vun_syntax::literal::{Mismatch, Name, Path};

use super::paths::{Files, Named as _};

use super::written::{self, Entry, TokenRef, Written};
use super::{Contents, Field, Kind, Kinds, Model, OperandContext, Restriction, Table, Term, Token, Variant};
use crate::generate::{Read, Source};
use crate::grammar::{GENERATOR, SHAPE, UNRESOLVED};

impl Model {
    /// The model of the grammar's files among `sources`.
    pub(crate) fn read(sources: &[Source<'_>]) -> Read<Self> {
        let files = Files::of(sources);
        let written = files.read::<Written>()?;
        let tokens = Self::kinds(&written.tokens.0, |_| Ok(None))?;
        let mut model = Self {
            tokens,
            nodes: Kinds { variants: Vec::new(), lists: Vec::new() },
            fields: Vec::new(),
            slots: Vec::new(),
            parts: Vec::new(),
            contents: BTreeMap::new(),
            operand_contexts: Vec::new(),
            preferred: Vec::new(),
            unterminated: Vec::new(),
            transparent: Vec::new(),
            line_ends: Vec::new(),
            precedence: Vec::new(),
            infix: Vec::new(),
            delimiters: Vec::new(),
            contextual: Vec::new(),
            infix_kind: None,
        };
        // Kinds are named before productions resolve, which may name any of them.
        let unresolved = Self::kinds(&written.nodes.0, |_| Ok(None))?;
        model.nodes = Kinds { variants: unresolved.variants, lists: unresolved.lists };
        let entries = written.nodes.0.iter().flat_map(|(_, entries)| entries);
        for (i, entry) in entries.enumerate() {
            let Some(terms) = &entry.production else { continue };
            let resolved =
                terms.iter().map(|t| model.term(&files, &entry.name, t)).collect::<Read<Vec<_>>>()?;
            model.nodes.variants[i].production = Some(resolved);
        }
        for i in 0..model.nodes.variants.len() {
            let slots = match model.nodes.variants[i].production.as_deref() {
                Some(terms) => Term::slots(terms).map_err(|field| {
                    let msg = Msg::new()
                        .text("the field ")
                        .code(model.field_name(field))
                        .text(" stands apart twice");
                    GENERATOR.error(SHAPE, &model.nodes.variants[i].name, msg)
                })?,
                None => Vec::new(),
            };
            model.slots.push(slots);
        }
        model.tables(&written)?;
        Ok(model)
    }

    /// A kind's variants and lists from its entries by list, the productions resolved by `resolve`.
    fn kinds<I: From<usize>, T>(
        lists: &[(String, Vec<Entry>)],
        resolve: impl Fn(&Entry) -> Read<Option<Vec<T>>>,
    ) -> Read<Kinds<I, T>> {
        let (mut variants, mut named) = (Vec::new(), Vec::new());
        for (list, entries) in lists {
            let mut members = Vec::new();
            for entry in entries {
                members.push(I::from(variants.len()));
                variants.push(Variant {
                    name: entry.name.clone(),
                    text: entry.text.clone(),
                    production: resolve(entry)?,
                });
            }
            named.push((list.clone(), members));
        }
        Ok(Kinds { variants, lists: named })
    }

    fn term(&mut self, files: &Files<'_, '_>, owner: &str, term: &written::Term) -> Read<Term> {
        let all = |terms: &[written::Term], model: &mut Self| {
            terms.iter().map(|t| model.term(files, owner, t)).collect::<Read<Vec<_>>>()
        };
        Ok(match term {
            written::Term::Text(text) => match self.all_tokens().find(|&t| self.text(t) == Some(text)) {
                Some(token) => Term::Tokens(vec![token]),
                None if text.chars().all(|c| c.is_alphanumeric() || c == '_') => {
                    Term::Words(vec![text.clone()])
                }
                None => {
                    let msg = Msg::new().text("no token is spelled ").code(text);
                    return Err(GENERATOR.error(UNRESOLVED, owner, msg));
                }
            },
            written::Term::Token(refs) => {
                let mut tokens = Vec::new();
                for r in refs {
                    match r {
                        TokenRef::Kind(name) => tokens.push(self.token_named(owner, name)?),
                        TokenRef::Table(path) => tokens.extend(self.tokens_of(files, path)?),
                    }
                }
                Term::Tokens(tokens)
            }
            written::Term::Word(path) => Term::Words(files.words(path)?),
            written::Term::Field(name) => Term::Field(self.intern(name), None),
            written::Term::Of(field, path) => {
                Term::Field(self.intern(field), Some(Restriction::Of(self.kind_table(files, path)?)))
            }
            written::Term::Ending(field, path) => {
                Term::Field(self.intern(field), Some(Restriction::Ending(self.kind_table(files, path)?)))
            }
            written::Term::Starting(field, path) => {
                let members = self.tokens_of(files, path)?;
                Term::Field(
                    self.intern(field),
                    Some(Restriction::Starting(Table { name: path.name(), members })),
                )
            }
            written::Term::Optional(terms) => Term::Optional(all(terms, self)?),
            written::Term::Many(terms) => Term::Many(all(terms, self)?),
            written::Term::Choice(terms) => Term::Choice(all(terms, self)?),
            written::Term::Seq(terms) => Term::Sequence(all(terms, self)?),
            written::Term::Separated(item, separator) => Term::Separated(
                Box::new(self.term(files, owner, item)?),
                Box::new(self.term(files, owner, separator)?),
            ),
        })
    }

    /// The field `name`, numbered where it first stands.
    fn intern(&mut self, name: &Name) -> Field {
        self.field(&name.0).unwrap_or_else(|| {
            self.fields.push(name.0.clone());
            Field(self.fields.len() - 1)
        })
    }

    pub(super) fn token_named(&self, owner: &str, name: &Name) -> Read<Token> {
        self.token(&name.0).ok_or_else(|| unresolved(owner, "no token kind is named ", &name.0))
    }

    pub(super) fn kind_named(&self, owner: &str, name: &Name) -> Read<Kind> {
        self.kind(&name.0).ok_or_else(|| unresolved(owner, "no node kind is named ", &name.0))
    }

    pub(super) fn field_named(&self, owner: &str, name: &Name) -> Read<Field> {
        self.field(&name.0).ok_or_else(|| unresolved(owner, "no production has the field ", &name.0))
    }

    pub(super) fn kinds_named(&self, owner: &str, names: &[Name]) -> Read<Vec<Kind>> {
        names.iter().map(|n| self.kind_named(owner, n)).collect()
    }

    /// The token kinds the table or list at `path` names, each once, in the order first named,
    /// leaving out names of other things, as a table's columns of how to space or group its
    /// tokens.
    pub(super) fn tokens_of(&self, files: &Files<'_, '_>, path: &Path) -> Read<Vec<Token>> {
        let mut tokens = Vec::new();
        for token in files.names(path)?.iter().filter_map(|n| self.token(n)) {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }
        Ok(tokens)
    }

    fn kind_table(&self, files: &Files<'_, '_>, path: &Path) -> Read<Table<Kind>> {
        let names: Vec<Name> = files.names(path)?.into_iter().map(Name).collect();
        Ok(Table { name: path.name(), members: self.kinds_named(&path.name(), &names)? })
    }

    /// The tables the generator interprets, resolved.
    fn tables(&mut self, written: &Written) -> Read<()> {
        let kinds = |model: &Self, owner: &str, names: &[Name]| model.kinds_named(owner, names);
        self.parts = kinds(self, "parts", &written.parts)?;
        self.unterminated = kinds(self, "unterminated", &written.unterminated)?;
        self.transparent = written
            .transparent
            .iter()
            .map(|row| {
                let tokens =
                    row.tokens.iter().map(|t| self.token_named("transparent", t)).collect::<Read<_>>()?;
                Ok((self.kind_named("transparent", &row.kind)?, tokens))
            })
            .collect::<Read<_>>()?;
        self.line_ends = kinds(self, "line_ends", &written.line_ends)?;
        self.preferred =
            written.preferred.iter().map(|set| kinds(self, "preferred", set)).collect::<Read<_>>()?;
        self.precedence =
            written.precedence.iter().map(|row| kinds(self, "precedence", row)).collect::<Read<_>>()?;
        for (rows, operands) in [(&written.admits, true), (&written.holds, false)] {
            for row in rows {
                let listed = kinds(self, "contents", &row.kinds)?;
                for field in &row.fields {
                    let field = self.field_named("contents", field)?;
                    let entry = self
                        .contents
                        .entry(field)
                        .or_insert_with(|| Contents { kinds: Vec::new(), operands });
                    entry.kinds.extend(&listed);
                }
            }
        }
        for row in &written.operand_contexts {
            let fields =
                row.fields.iter().map(|f| self.field_named("operand_contexts", f)).collect::<Read<_>>()?;
            self.operand_contexts
                .push(OperandContext { fields, kinds: kinds(self, "operand_contexts", &row.kinds)? });
        }
        self.infix = written
            .infix
            .iter()
            .map(|level| level.tokens.iter().map(|t| self.token_named("infix", t)).collect())
            .collect::<Read<_>>()?;
        self.delimiters = written
            .delimiters
            .iter()
            .map(|d| {
                Ok((self.token_named("delimiters", &d.open)?, self.token_named("delimiters", &d.close)?))
            })
            .collect::<Read<_>>()?;
        self.contextual = written.contextual.iter().map(|c| (c.text.clone(), c.context)).collect();
        let operators: Vec<Token> = self.infix.iter().flatten().copied().collect();
        let spread = self.produced().find_map(|(kind, terms)| {
            let spread =
                Term::all_leaves(terms).into_iter().any(|l| matches!(l, Term::Tokens(t) if *t == operators));
            spread.then_some(kind)
        });
        self.infix_kind = spread;
        Ok(())
    }
}

impl From<usize> for Token {
    fn from(i: usize) -> Self {
        Self(i)
    }
}

impl From<usize> for Kind {
    fn from(i: usize) -> Self {
        Self(i)
    }
}

pub(super) fn unresolved(owner: &str, what: &'static str, name: &str) -> Box<vun_diag::Diagnostic> {
    GENERATOR.error(UNRESOLVED, owner, Msg::new().text(what).code(name))
}

pub(super) fn mismatch(owner: &str, m: &Mismatch) -> Box<vun_diag::Diagnostic> {
    Box::new(
        (*GENERATOR.error(SHAPE, owner, Msg::new().text("does not have the grammar's shape"))).caused_by(m),
    )
}
