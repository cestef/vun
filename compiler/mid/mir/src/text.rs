//! The MIR text format: locals by number (`_0` is the result), blocks by number, each statement
//! and terminator on its own line, and in dumps the source position of each line's origin.

use std::fmt;

use overt_index::Idx;
use overt_origin::OriginId;
use overt_span::LineCol;

use crate::body::{Aggregate, Body, Operand, Rvalue, TerminatorKind};
use crate::place::Place;

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_{}", self.local.index())
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Place(place) => place.fmt(f),
            Self::Const(value) => write!(f, "const {value}"),
        }
    }
}

impl fmt::Display for Rvalue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = |op: overt_grammar::TokenKind| op.text().unwrap_or("?");
        match self {
            Self::Use(operand) => operand.fmt(f),
            Self::Binary { op, left, right } => write!(f, "{left} {} {right}", text(*op)),
            Self::Unary { op, operand } => write!(f, "{}{operand}", text(*op)),
            Self::Range { op, lo, hi: Some(hi) } => write!(f, "{lo}{}{hi}", text(*op)),
            Self::Range { op, lo, hi: None } => write!(f, "{lo}{}", text(*op)),
            Self::Comptime(expr) => write!(f, "comptime e{}", expr.index()),
            Self::Aggregate { kind, parts } => {
                let (open, close) = match kind {
                    Aggregate::Struct { ty, .. } | Aggregate::Variant { ty, .. } => {
                        write!(f, "{ty}")?;
                        if let Aggregate::Variant { name, .. } = kind {
                            write!(f, ".{name}")?;
                        }
                        ("{ ", " }")
                    }
                    Aggregate::Tuple => ("(", ")"),
                    Aggregate::Array => ("[", "]"),
                };
                f.write_str(open)?;
                for (i, part) in parts.iter().enumerate() {
                    write!(f, "{}{part}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(close)
            }
            Self::Field { operand, name } => write!(f, "{operand}.{name}"),
            Self::Index { operand, index } => write!(f, "{operand}[{index}]"),
            Self::Narrow { operand, ty, narrowing } => {
                write!(f, "{operand} narrowed to {ty} ({narrowing:?})")
            }
            Self::Call { callee, args } => {
                write!(f, "{callee}(")?;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, "{}{arg}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(")")
            }
        }
    }
}

/// A body as text; with a locator, each line ends with the source position of its origin, as
/// `overt dump mir` prints it (spec 10, section 1).
pub struct Text<'b> {
    body: &'b Body,
    locate: Option<&'b dyn Fn(OriginId) -> LineCol>,
}

impl Body {
    /// This body's text with the origin of each line, found by `locate`.
    #[must_use]
    pub fn dump<'b>(&'b self, locate: &'b dyn Fn(OriginId) -> LineCol) -> Text<'b> {
        Text { body: self, locate: Some(locate) }
    }
}

impl Text<'_> {
    /// Ends a line with the position of `origin`, when locating.
    fn end(&self, f: &mut fmt::Formatter<'_>, origin: OriginId) -> fmt::Result {
        match self.locate {
            Some(locate) => {
                let at = locate(origin);
                writeln!(f, " @{}:{}", at.line, at.col)
            }
            None => writeln!(f),
        }
    }
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = self.body;
        f.write_str("fn(")?;
        for (i, &p) in body.params.iter().enumerate() {
            let name = body.locals[p].name.map_or("", |n| n.as_str());
            write!(f, "{}{}: {name}", if i > 0 { ", " } else { "" }, Place { local: p })?;
        }
        write!(f, ") -> {} {{", Place { local: body.ret })?;
        self.end(f, body.origin)?;
        for (id, block) in body.blocks.iter() {
            writeln!(f, "  bb{}:", id.index())?;
            for stmt in &block.stmts {
                write!(f, "    {} = {}", stmt.place, stmt.value)?;
                self.end(f, stmt.origin)?;
            }
            match &block.term.kind {
                TerminatorKind::Return => write!(f, "    return")?,
                TerminatorKind::Goto(to) => write!(f, "    goto bb{}", to.index())?,
                TerminatorKind::Branch { cond, then, otherwise } => {
                    write!(f, "    if {cond} goto bb{} else bb{}", then.index(), otherwise.index())?;
                }
            }
            self.end(f, block.term.origin)?;
        }
        f.write_str("}")
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Text { body: self, locate: None }.fmt(f)
    }
}
