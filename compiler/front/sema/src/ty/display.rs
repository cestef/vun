//! Types as diagnostics show them: integers by their range, primitives by name with their
//! arguments, literals by kind.

use std::fmt;

use super::{NominalKind, TyKind, Value};
use crate::int::Extended;

impl fmt::Display for TyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(range) = self.interval() {
            return match range.hi() {
                Extended::PosInf => write!(f, "Int({}..)", Bound(range.lo())),
                hi => write!(f, "Int({}..={})", Bound(range.lo()), Bound(hi)),
            };
        }
        match self {
            Self::Primitive(p, args) if args.is_empty() => f.write_str(p.name()),
            Self::Primitive(p, args) => {
                write!(f, "{}(", p.name())?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    match arg {
                        Value::Type(ty) => write!(f, "{ty}")?,
                        Value::Int(n) => write!(f, "{n}")?,
                        _ => f.write_str("..")?,
                    }
                }
                f.write_str(")")
            }
            Self::Nominal(n) => f.write_str(match n.kind {
                NominalKind::Struct => "struct { .. }",
                NominalKind::Enum => "enum { .. }",
            }),
            Self::Tuple(items) => {
                f.write_str("(")?;
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}{item}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(")")
            }
            Self::Array { len, elem } => write!(f, "[{len}]{elem}"),
        }
    }
}

/// A finite range bound, as written.
struct Bound<'a>(&'a Extended);

impl fmt::Display for Bound<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Extended::Finite(n) => write!(f, "{n}"),
            Extended::NegInf | Extended::PosInf => f.write_str("_"),
        }
    }
}
