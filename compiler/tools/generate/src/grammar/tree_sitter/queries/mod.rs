//! The queries editors read beside the grammar: highlights, folds, indents, brackets, the outline
//! and text objects. Each is built once from the grammar, as typed patterns and captures, and
//! written in the dialect of each editor, which names captures its own way and may let the first
//! matching pattern win rather than the last.

mod highlights;
mod structure;

use vun_tree_sitter::{Pattern, Query};

use super::rules::Rules;
use crate::grammar::model::Captures;

/// What a pattern captures a node as, as `editors.vun` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_syntax::literal::FromLiteral)]
pub(crate) enum Capture {
    Keyword,
    Operator,
    Bracket,
    Delimiter,
    Number,
    String,
    Character,
    Comment,
    Documentation,
    Label,
    Function,
    Call,
    Method,
    Property,
    Variant,
    Parameter,
    Type,
    Variable,
    Builtin,
    /// A node folded as one.
    Fold,
    /// A node whose lines inside are indented one level more.
    Indent,
    /// A token closing what an indent opened.
    Outdent,
    /// An outline entry, and its name.
    Item,
    Name,
    /// Text objects: a function, its body, a type, a parameter, a comment.
    FunctionAround,
    FunctionInside,
    TypeAround,
    ParameterInside,
    CommentAround,
    /// A pair of brackets.
    Open,
    Close,
}

/// A query file an editor reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum File {
    Highlights,
    Folds,
    Indents,
    Brackets,
    Outline,
    TextObjects,
}

impl File {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Highlights => "highlights.scm",
            Self::Folds => "folds.scm",
            Self::Indents => "indents.scm",
            Self::Brackets => "brackets.scm",
            Self::Outline => "outline.scm",
            Self::TextObjects => "textobjects.scm",
        }
    }
}

/// How an editor reads queries: the files it reads and what it names each capture. Every editor
/// lets the last pattern matching a node win, so patterns go from the most general to the most
/// specific (Helix too, since 25.07).
pub(crate) struct Dialect {
    pub(crate) files: &'static [File],
    pub(crate) capture: fn(Capture) -> Option<&'static str>,
}

/// A pattern with typed captures; lists of them go from the most general to the most specific.
pub(crate) type Typed = Pattern<Capture>;

impl Dialect {
    /// The query `file` of the grammar in this dialect, leaving out the patterns capturing
    /// something the dialect has no name for.
    pub(super) fn query(&self, rules: &Rules<'_>, file: File) -> Query<&'static str> {
        let typed = match file {
            File::Highlights => highlights::patterns(rules),
            File::Folds => structure::folds(rules),
            File::Indents => structure::indents(rules),
            File::Brackets => structure::brackets(rules),
            File::Outline => structure::outline(rules),
            File::TextObjects => structure::text_objects(rules),
        };
        Query { patterns: typed.into_iter().filter_map(|p| p.map(&self.capture)).collect() }
    }
}

/// Patterns of nodes captured whole, then of nodes captured where they stand.
fn captured(rules: &Rules<'_>, captures: &Captures) -> Vec<Typed> {
    let model = rules.model;
    let mut patterns: Vec<Typed> = captures
        .nodes
        .iter()
        .map(|(nodes, capture)| {
            Pattern::alternatives(nodes.iter().map(|&n| Pattern::node(model.node_name(n)))).capture(*capture)
        })
        .collect();
    for place in &captures.places {
        let node = Pattern::node(model.kind_name(place.node)).capture(place.capture);
        let held = match place.inner {
            Some(inner) => Pattern::any().child(Some(model.field_name(inner)), node),
            None => node,
        };
        for &kind in &place.kinds {
            patterns.push(
                Pattern::node(model.kind_name(kind)).child(Some(model.field_name(place.field)), held.clone()),
            );
        }
    }
    patterns
}
