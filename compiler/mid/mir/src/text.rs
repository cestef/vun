//! The MIR text format: locals by number (`_0` is the result), blocks by number, each statement
//! and terminator on its own line.

use std::fmt;

use overt_index::Idx;
use overt_sema::ty::NominalKind;

use crate::body::{Body, Operand, Rvalue, TerminatorKind};
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
            Self::Const(value) => write!(f, "const {value:?}"),
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
            Self::Nominal { kind: NominalKind::Struct, .. } => f.write_str("struct { .. }"),
            Self::Nominal { kind: NominalKind::Enum, .. } => f.write_str("enum { .. }"),
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

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fn(")?;
        for (i, &p) in self.params.iter().enumerate() {
            let name = self.locals[p].name.map_or("", |n| n.as_str());
            write!(f, "{}{}: {name}", if i > 0 { ", " } else { "" }, Place { local: p })?;
        }
        writeln!(f, ") -> {} {{", Place { local: self.ret })?;
        for (id, block) in self.blocks.iter() {
            writeln!(f, "  bb{}:", id.index())?;
            for stmt in &block.stmts {
                writeln!(f, "    {} = {}", stmt.place, stmt.value)?;
            }
            match block.term.kind {
                TerminatorKind::Return => writeln!(f, "    return")?,
                TerminatorKind::Goto(to) => writeln!(f, "    goto bb{}", to.index())?,
            }
        }
        f.write_str("}")
    }
}
