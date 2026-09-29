//! The queries editors read beside the grammar: highlights, folds, indents, brackets, the outline
//! and text objects. Each is built once from the grammar, as typed patterns and captures, and
//! written in the dialect of each editor, which names captures its own way and may let the first
//! matching pattern win rather than the last.

mod highlights;
mod structure;

use vun_tree_sitter::{Pattern, Query};

use super::rules::Rules;
use crate::generate::Read;

/// What a pattern captures a node as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// How an editor reads queries: the files it reads, what it names each capture, and whether
/// the first pattern matching a node wins, rather than the last.
pub(crate) struct Dialect {
    pub(crate) files: &'static [File],
    pub(crate) capture: fn(Capture) -> Option<&'static str>,
    pub(crate) first_wins: bool,
}

/// A pattern with typed captures; lists of them go from the most general to the most specific.
pub(crate) type Typed = Pattern<Capture>;

impl Dialect {
    /// The query `file` of the grammar in this dialect, leaving out the patterns capturing
    /// something the dialect has no name for.
    pub(super) fn query(&self, rules: &Rules<'_, '_>, file: File) -> Read<Query<&'static str>> {
        let mut typed = match file {
            File::Highlights => highlights::patterns(rules)?,
            File::Folds => structure::folds(rules),
            File::Indents => structure::indents(rules),
            File::Brackets => structure::brackets(rules),
            File::Outline => structure::outline(),
            File::TextObjects => structure::text_objects(),
        };
        if self.first_wins {
            typed.reverse();
        }
        Ok(Query { patterns: typed.into_iter().filter_map(|p| p.map(&self.capture)).collect() })
    }
}
