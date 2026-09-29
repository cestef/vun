//! A tree-sitter rule per node kind, from its production: its tokens as strings or lexical rules,
//! its fields named, each holding the kind it is named after or any operand, and a precedence
//! from the table `precedence` for the forms open at an edge.

use std::collections::BTreeMap;

use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Literal};

use super::{TREE_SITTER, lexical};

mod infix;
mod levels;
mod restrict;
use crate::generate::Read;
use crate::grammar::production::{Resolved, Term, Tokens};
use crate::grammar::{Enum, Grammar};
use vun_tree_sitter::{Assoc, Rule};

const MISSING: DiagId = DiagId::new("generate.tree_sitter.missing");

/// Names the rules read, as `nodes.vun` and `grammar.vun` declare them.
const NODES: &str = "NodeKind";
/// The table of node kinds that are never operands.
const PARTS: &str = "parts";
/// The kind standing for what recovery skipped, which tree-sitter has its own of.
const ERROR: &str = "error";
/// The list of token kinds ending a file, which the root's production names and tree-sitter implies.
const END: &str = "end";
/// The tables of what fields admit besides operands and hold alone, and their rows' columns.
const ADMITS: &str = "admits";
const HOLDS: &str = "holds";
const FIELDS: &str = "fields";
const KINDS: &str = "kinds";
/// The table of kinds that end with their line.
const LINE_ENDS: &str = "line_ends";
/// The table of kinds counting as their last child where a table names kinds.
const TRANSPARENT: &str = "transparent";
/// The table of kinds whose productions overlap.
const PREFERRED: &str = "preferred";
/// The table of precedence levels, and the table of infix levels the row naming `INFIX` stands for.
const PRECEDENCE: &str = "precedence";
pub(super) const INFIX: &str = "infix";
/// The field of a row of the infix table listing its operators.
const TOKENS: &str = "tokens";

/// The hidden rule of any operand.
pub(super) const OPERAND: &str = "_operand";

/// How the rules are built: the node kinds, their resolved tokens and precedences.
pub(super) struct Rules<'g, 's> {
    nodes: &'g Enum<'s>,
    /// Each kind with a production, and its terms resolved.
    pub(super) produced: Vec<(&'s str, Vec<Term<'s, Resolved<'s>>>)>,
    pub(super) tokens: Tokens<'g, 's>,
    pub(super) decls: &'g [Decl<'s>],
    /// Each kind in `precedence` with its level, loosest lowest.
    levels: BTreeMap<&'s str, i32>,
    /// The level of the infix table's first row.
    infix_base: i32,
    /// The groups of kinds that overlap, most preferred first.
    pub(super) preferred: Vec<Vec<&'s str>>,
    /// Each field held or admitting kinds, with them, and whether it also holds operands.
    contents: BTreeMap<&'s str, (Vec<&'s str>, bool)>,
    /// Hidden rules of transparent kinds ending with the kinds of a table, by name.
    wrapped: std::cell::RefCell<Vec<Named>>,
    /// The unspelled tokens the rules name, which need lexical rules.
    pub(super) lexical: std::cell::RefCell<Vec<&'s str>>,
}

/// A generated rule: its name and body.
pub(super) type Named = (String, Rule);

impl<'g, 's> Rules<'g, 's> {
    pub(super) fn of(grammar: &'g Grammar<'s>, decls: &'g [Decl<'s>]) -> Read<Self> {
        let nodes = grammar.enums.iter().find(|e| e.name == NODES).ok_or_else(|| missing(NODES))?;
        let tokens = Tokens::of(grammar, decls, nodes)?;
        let rows = table(decls, PRECEDENCE)?.as_list().ok_or_else(|| missing(PRECEDENCE))?;
        let infix_levels = table(decls, INFIX)?.as_list().map_or(0, <[_]>::len);
        let (mut levels, mut level, mut infix_base) = (BTreeMap::new(), 1, 0);
        for row in rows {
            let kinds = row.as_list().ok_or_else(|| missing(PRECEDENCE))?;
            for kind in kinds.iter().filter_map(Literal::as_name) {
                levels.insert(kind, level);
                if kind == INFIX {
                    infix_base = level;
                }
            }
            level += if kinds.iter().any(|k| k.as_name() == Some(INFIX)) {
                i32::try_from(infix_levels).unwrap_or(i32::MAX)
            } else {
                1
            };
        }
        let mut produced = Vec::new();
        for v in nodes.variants.iter().filter(|v| v.name != ERROR) {
            if let Some(terms) = &v.production {
                produced.push((v.name, tokens.resolve(v.name, terms)?));
            }
        }
        let mut contents: BTreeMap<&'s str, (Vec<&'s str>, bool)> = BTreeMap::new();
        for (name, operands) in [(ADMITS, true), (HOLDS, false)] {
            for row in table(decls, name)?.as_list().ok_or_else(|| missing(name))? {
                let kinds = column(row, KINDS)?;
                for field in column(row, FIELDS)? {
                    contents.entry(field).or_insert_with(|| (Vec::new(), operands)).0.extend(&kinds);
                }
            }
        }
        let rows = table(decls, PREFERRED)?.as_list().ok_or_else(|| missing(PREFERRED))?;
        let preferred = rows
            .iter()
            .map(|r| r.as_list().unwrap_or_default().iter().filter_map(Literal::as_name).collect())
            .collect();
        let (wrapped, lexical) = (std::cell::RefCell::default(), std::cell::RefCell::default());
        Ok(Self { nodes, produced, tokens, decls, levels, infix_base, preferred, contents, wrapped, lexical })
    }

    /// The root first, then a rule per other kind with a production, then `_operand`.
    pub(super) fn all(&self) -> Read<Vec<Named>> {
        let parts = self.kinds(PARTS)?;
        let (roots, others): (Vec<_>, Vec<_>) =
            self.produced.iter().partition(|(_, terms)| self.is_root(terms));
        let mut rules = Vec::new();
        for (name, terms) in roots.iter().chain(&others) {
            rules.push(((*name).to_owned(), self.rule(name, terms)?));
        }
        let operands =
            others.iter().filter(|(name, _)| !parts.contains(name)).map(|(name, _)| Rule::symbol(name));
        rules.push((OPERAND.to_owned(), Rule::choice(operands.collect())));
        Ok(rules)
    }

    /// The names a table of kinds lists.
    fn kinds(&self, name: &str) -> Read<Vec<&'s str>> {
        let kinds = table(self.decls, name)?.as_list().ok_or_else(|| missing(name))?;
        Ok(kinds.iter().filter_map(Literal::as_name).collect())
    }

    /// The token kinds ending a file.
    fn ends(&self) -> &[&'s str] {
        self.tokens.kind.lists.iter().find(|(l, _)| *l == END).map_or(&[], |(_, e)| e.as_slice())
    }

    /// Whether `terms` name a token ending a file.
    fn is_root(&self, terms: &[Term<'s, Resolved<'s>>]) -> bool {
        let mut leaves = Vec::new();
        for t in terms {
            t.leaves(&mut leaves);
        }
        leaves
            .iter()
            .any(|l| matches!(l, Resolved::Tokens(names) if names.iter().any(|n| self.ends().contains(n))))
    }

    fn rule(&self, name: &str, terms: &[Term<'s, Resolved<'s>>]) -> Read<Rule> {
        if name == INFIX {
            return self.infix(terms);
        }
        // In a branch, a token read by hand is a node of its own, named after its kind: `doc_comment`.
        let named = !Term::slots(name, terms)?.is_empty();
        let mut body: Vec<Rule> = terms.iter().filter_map(|t| self.term(t, named)).collect();
        if self.kinds(LINE_ENDS)?.contains(&name) {
            body.push(Rule::symbol(super::scanner::LINE_END));
        }
        let mut body = Rule::seq(body);
        let rank = self.rank(name);
        if rank > 0 {
            body = body.dynamic(-rank);
        }
        Ok(match self.levels.get(name) {
            Some(&level) => body.prec(self.assoc(terms), level),
            None => body,
        })
    }

    /// A term as a rule; `None` for one tree-sitter implies, the end of a file.
    fn term(&self, term: &Term<'s, Resolved<'s>>, named: bool) -> Option<Rule> {
        let all = |terms: &[Term<'s, Resolved<'s>>]| {
            Rule::seq(terms.iter().filter_map(|t| self.term(t, named)).collect())
        };
        Some(match term {
            Term::Leaf(Resolved::Tokens(kinds)) => {
                let spelled: Vec<_> = kinds.iter().filter(|k| !self.ends().contains(k)).collect();
                if spelled.is_empty() {
                    return None;
                }
                Rule::choice(spelled.into_iter().map(|k| self.token(k, named)).collect())
            }
            Term::Leaf(Resolved::Words(words)) => {
                Rule::choice(words.iter().map(|w| Rule::Text(w.to_string())).collect())
            }
            Term::Field(field, None) => self.content(field).field(field),
            // Kinds a production takes apart are complete there: an entry `for {}` goes on with no
            // operator, and where a list could also read it as a plain entry, the trees are the same.
            Term::Field(field, Some(restriction)) => {
                self.restricted_to(*restriction)?.prec(Assoc::None, self.above_operators()).field(field)
            }
            Term::Optional(terms) => all(terms).optional(),
            Term::Many(terms) => all(terms).repeat(),
            Term::Choice(terms) => Rule::choice(terms.iter().filter_map(|t| self.term(t, named)).collect()),
            Term::Sequence(terms) => all(terms),
            Term::Separated(item, separator) => {
                let (item, separator) = (self.term(item, named)?, self.term(separator, named)?);
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
    /// operand.
    fn content(&self, field: &str) -> Rule {
        if self.nodes.variants.iter().any(|v| v.name == field) {
            return Rule::symbol(field);
        }
        match self.contents.get(field) {
            Some((kinds, operands)) => {
                let mut rules: Vec<Rule> = kinds.iter().map(|k| Rule::symbol(k)).collect();
                if *operands {
                    rules.push(Rule::symbol(OPERAND));
                }
                Rule::choice(rules)
            }
            None => Rule::symbol(OPERAND),
        }
    }

    /// The token kind `name`: its spelling, or the hidden rule of an unspelled one, as a node
    /// named after its kind if `named`.
    fn token(&self, name: &str, named: bool) -> Rule {
        let variant = self.tokens.kind.variants.iter().find(|v| v.name == name);
        if let Some(text) = variant.and_then(|v| v.text.as_deref()) {
            return Rule::Text(text.to_owned());
        }
        let mut lexical = self.lexical.borrow_mut();
        if let Some(v) = variant.filter(|v| !lexical.contains(&v.name)) {
            lexical.push(v.name);
        }
        let rule = Rule::symbol(&hidden(name));
        if named { rule.alias(name) } else { rule }
    }

    /// The lexical rules of the unspelled tokens named.
    pub(super) fn lexical(&self) -> Read<Vec<Named>> {
        self.lexical
            .borrow()
            .iter()
            .map(|name| Ok((hidden(name), lexical::rule(name).ok_or_else(|| missing(name))?)))
            .collect()
    }
}

/// The name of the hidden rule of a token kind: `_ident`.
pub(super) fn hidden(name: &str) -> String {
    ["_", name].concat()
}

/// The names a column of a table's row lists.
pub(super) fn column<'s>(row: &Literal<'s>, name: &str) -> Read<Vec<&'s str>> {
    let list = row.get(name).and_then(Literal::as_list).ok_or_else(|| missing(name))?;
    Ok(list.iter().filter_map(Literal::as_name).collect())
}

pub(super) fn table<'d, 's>(decls: &'d [Decl<'s>], name: &str) -> Read<&'d Literal<'s>> {
    decls.iter().find(|d| d.name == name).map(|d| &d.value).ok_or_else(|| missing(name))
}

pub(super) fn missing(what: &str) -> Box<vun_diag::Diagnostic> {
    TREE_SITTER.error(
        MISSING,
        what,
        Msg::new().text("the tree-sitter grammar reads it, but the grammar has none"),
    )
}
