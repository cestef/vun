//! Types as diagnostics show them: integers by their range, primitives by name with their
//! arguments, literals by the declaration they are written in with its arguments, or by kind.

use std::fmt;

use super::{Nominal, NominalKind, TyKind, Value};
use crate::int::Extended;

impl fmt::Display for TyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(range) = self.interval() {
            return match range.hi() {
                Extended::PosInf => write!(f, "Int({}..)", Bound(range.lo())),
                hi => write!(f, "Int({}..={})", Bound(range.lo()), Bound(hi)),
            };
        }
        if let Some(sig) = self.signature() {
            return write!(f, "{} -> {}", Self::Tuple(sig.params), sig.result);
        }
        match self {
            Self::Primitive(p, args) => Applied(p.name(), args).fmt(f),
            Self::Nominal(Nominal { name: Some(name), instance, .. }) => {
                Applied(name.as_str(), instance.as_ref().map_or(&[], |i| &i.args)).fmt(f)
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

/// `name(args)`, or `name` alone without arguments: a primitive or a type-returning function
/// applied to compile-time values.
struct Applied<'a>(&'a str, &'a [Value]);

impl fmt::Display for Applied<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)?;
        if self.1.is_empty() {
            return Ok(());
        }
        f.write_str("(")?;
        for (i, arg) in self.1.iter().enumerate() {
            f.write_str(if i > 0 { ", " } else { "" })?;
            match arg {
                Value::Type(ty) => ty.fmt(f)?,
                Value::Int(n) => n.fmt(f)?,
                _ => f.write_str("..")?,
            }
        }
        f.write_str(")")
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
