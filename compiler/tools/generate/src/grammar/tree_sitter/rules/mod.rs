//! A tree-sitter rule per node kind, from its production: its tokens as strings or lexical rules,
//! its fields named, each holding the kind it is named after or any operand, and a precedence
//! from the table `precedence` for the forms open at an edge.

use std::cell::RefCell;
use std::collections::BTreeMap;

use vun_diag::{DiagId, Msg};

use super::{TREE_SITTER, lexical};

mod context;
mod infix;
mod levels;
mod restrict;
use crate::generate::Read;
use crate::grammar::model::{Editors, Field, Kind, Model, Term, Token};
use vun_tree_sitter::{Assoc, Rule};

const MISSING: DiagId = DiagId::new("generate.tree_sitter.missing");

/// The hidden rule of any operand.
pub(super) const OPERAND: &str = "_operand";

/// How the rules are built: the model, the editors' facts, and each kind's precedence.
pub(super) struct Rules<'m> {
    pub(super) model: &'m Model,
    pub(super) editors: &'m Editors,
    /// Each kind in `precedence` with its level, loosest lowest.
    levels: BTreeMap<Kind, i32>,
    /// The level of the loosest infix level.
    infix_base: i32,
    /// Hidden rules made for restricted fields, by name.
    wrapped: RefCell<Vec<Named>>,
    /// The unspelled tokens the rules name, which need lexical rules.
    lexical: RefCell<Vec<Token>>,
}

/// A generated rule: its name and body.
pub(super) type Named = (String, Rule);

/// Where a term stands: whether a token read by hand is a node of its own there, whether its
/// kind has a precedence level, which then decides where a restricted field ends, and the operand
/// context it is read in, if any, by its position in `operand_contexts`.
#[derive(Clone, Copy)]
pub(super) struct At {
    named: bool,
    ranked: bool,
    context: Option<usize>,
}

impl At {
    /// In a branch of a kind with a level.
    pub(super) const RANKED: Self = Self { named: true, ranked: true, context: None };
}

impl<'m> Rules<'m> {
    pub(super) fn of(model: &'m Model, editors: &'m Editors) -> Self {
        let infix_levels = i32::try_from(model.infix.len()).unwrap_or(i32::MAX);
        let (mut levels, mut level, mut infix_base) = (BTreeMap::new(), 1, 0);
        for row in &model.precedence {
            for &kind in row {
                levels.insert(kind, level);
                if Some(kind) == model.infix_kind {
                    infix_base = level;
                }
            }
            level += if row.iter().any(|&k| Some(k) == model.infix_kind) { infix_levels } else { 1 };
        }
        Self { model, editors, levels, infix_base, wrapped: RefCell::default(), lexical: RefCell::default() }
    }

    /// The root first, then a rule per other kind with a production, then `_operand`.
    pub(super) fn all(&self) -> Vec<Named> {
        let error = self.editors.tree_sitter.error;
        let produced = self.model.produced().filter(|&(kind, _)| kind != error);
        let (roots, others): (Vec<_>, Vec<_>) = produced.partition(|(_, terms)| self.is_root(terms));
        let mut rules = Vec::new();
        for (kind, terms) in roots.iter().chain(&others) {
            rules.push((self.model.kind_name(*kind).to_owned(), self.rule(*kind, terms, None)));
        }
        let operands = others.iter().filter(|(kind, _)| !self.model.parts.contains(kind));
        let operands = operands.map(|(kind, _)| Rule::symbol(self.model.kind_name(*kind)));
        rules.push((OPERAND.to_owned(), Rule::choice(operands.collect())));
        rules
    }

    /// Whether `terms` name a token ending a file.
    fn is_root(&self, terms: &[Term]) -> bool {
        let end = &self.editors.tree_sitter.end;
        Term::all_leaves(terms)
            .iter()
            .any(|l| matches!(l, Term::Tokens(tokens) if tokens.iter().any(|t| end.contains(t))))
    }

    /// The rule of `kind`, its operands read in `context` if one is given.
    fn rule(&self, kind: Kind, terms: &[Term], context: Option<usize>) -> Rule {
        if Some(kind) == self.model.infix_kind {
            return self.infix(terms, context);
        }
        // In a branch, a token read by hand is a node of its own, named after its kind: `doc_comment`.
        let named = !self.model.slots(kind).is_empty();
        let at = At { named, ranked: self.levels.contains_key(&kind), context };
        let mut body: Vec<Rule> = terms.iter().filter_map(|t| self.term(t, at)).collect();
        if self.model.line_ends.contains(&kind) {
            body.push(Rule::symbol(super::scanner::LINE_END));
        }
        let mut body = Rule::seq(body);
        let rank = self.rank(kind);
        if rank > 0 {
            body = body.dynamic(-rank);
        }
        match self.levels.get(&kind) {
            Some(&level) => body.prec(self.assoc(terms), level),
            None => body,
        }
    }

    /// A term as a rule; `None` for one tree-sitter implies, the end of a file.
    fn term(&self, term: &Term, at: At) -> Option<Rule> {
        let all = |terms: &[Term]| Rule::seq(terms.iter().filter_map(|t| self.term(t, at)).collect());
        Some(match term {
            Term::Tokens(tokens) => {
                let end = &self.editors.tree_sitter.end;
                let spelled: Vec<_> = tokens.iter().filter(|t| !end.contains(t)).collect();
                if spelled.is_empty() {
                    return None;
                }
                Rule::choice(spelled.into_iter().map(|&t| self.token(t, at.named)).collect())
            }
            Term::Words(words) => Rule::choice(words.iter().map(|w| Rule::Text(w.clone())).collect()),
            Term::Field(field, None) => self.content(*field, at.context).field(self.model.field_name(*field)),
            // In a kind with no level of its own, kinds a production takes apart are complete there:
            // an entry `for {}` goes on with no operator, and where a list could also read it as a
            // plain entry, the trees are the same. In one with a level, that level decides.
            Term::Field(field, Some(restriction)) => {
                let held = self.restricted_to(restriction);
                let held = if at.ranked { held } else { held.prec(Assoc::None, self.above_operators()) };
                held.field(self.model.field_name(*field))
            }
            Term::Optional(terms) => all(terms).optional(),
            Term::Many(terms) => all(terms).repeat(),
            Term::Choice(terms) => Rule::choice(terms.iter().filter_map(|t| self.term(t, at)).collect()),
            Term::Sequence(terms) => all(terms),
            Term::Separated(item, separator) => {
                let (item, separator) = (self.term(item, at)?, self.term(separator, at)?);
                Rule::seq(vec![
                    item.clone(),
                    Rule::seq(vec![separator.clone(), item]).repeat(),
                    separator.optional(),
                ])
                .optional()
            }
        })
    }

    /// What the field `field` holds: the kind it is named after, the kinds a table says, or any
    /// operand, read in the field's own context, else in `context`.
    fn content(&self, field: Field, context: Option<usize>) -> Rule {
        if let Some(kind) = self.model.named_kind(field) {
            return Rule::symbol(self.model.kind_name(kind));
        }
        let operand = match self.context_holding(field).or(context) {
            Some(context) => self.context_operand(context),
            None => Rule::symbol(OPERAND),
        };
        match self.model.contents.get(&field) {
            Some(contents) => {
                let mut rules: Vec<Rule> =
                    contents.kinds.iter().map(|&k| Rule::symbol(self.model.kind_name(k))).collect();
                if contents.operands {
                    rules.push(operand);
                }
                Rule::choice(rules)
            }
            None => operand,
        }
    }

    /// A token: its spelling, or the hidden rule of an unspelled one, as a node named after its
    /// kind if `named`.
    pub(super) fn token(&self, token: Token, named: bool) -> Rule {
        if let Some(text) = self.model.text(token) {
            return Rule::Text(text.to_owned());
        }
        let mut lexical = self.lexical.borrow_mut();
        if !lexical.contains(&token) {
            lexical.push(token);
        }
        let name = self.model.token_name(token);
        let rule = Rule::symbol(&hidden(name));
        if named { rule.alias(name) } else { rule }
    }

    /// The lexical rules of the unspelled tokens named.
    pub(super) fn lexical(&self) -> Read<Vec<Named>> {
        let rules = self.lexical.borrow();
        rules
            .iter()
            .map(|&t| {
                let name = self.model.token_name(t);
                Ok((hidden(name), lexical::rule(name).ok_or_else(|| missing(name))?))
            })
            .collect()
    }
}

/// The name of the hidden rule of a token kind: `_ident`.
pub(super) fn hidden(name: &str) -> String {
    ["_", name].concat()
}

pub(super) fn missing(what: &str) -> Box<vun_diag::Diagnostic> {
    TREE_SITTER.error(
        MISSING,
        what,
        Msg::new().text("the tree-sitter grammar reads it, but the grammar has none"),
    )
}
