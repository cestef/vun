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
        // Borrows and sequences as written: `&mut T`, `[4]u8`, `[u8]`.
        if let Some((mode, _, ty)) = self.borrowing() {
            return match mode {
                "" => write!(f, "&{ty}"),
                mode => write!(f, "&{mode} {ty}"),
            };
        }
        if let Some((len, elem)) = self.as_array() {
            return write!(f, "[{len}]{elem}");
        }
        if let Some(elem) = self.items() {
            return write!(f, "[{elem}]");
        }
        match self {
            Self::Primitive(p, args) => Applied(p.name(), args).fmt(f),
            Self::Nominal(Nominal { name: Some(name), instance, .. }) => {
                Applied(name.as_str(), instance.as_ref().map_or(&[], |i| &i.args)).fmt(f)
            }
            Self::Nominal(n) => f.write_str(match n.kind {
                NominalKind::Struct => "struct { .. }",
                NominalKind::Enum => "enum { .. }",
                NominalKind::Empty => "enum {}",
            }),
            Self::Tuple(items) => {
                f.write_str("(")?;
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}{item}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(")")
            }
            // `[x: u32, n: &mut u32] (u8) -> ()`: the captures, then the function.
            Self::Closure(closure) => {
                f.write_str("[")?;
                for (i, (name, ty)) in closure.captures.iter().enumerate() {
                    write!(f, "{}{name}: {ty}", if i > 0 { ", " } else { "" })?;
                }
                let sig = &closure.signature;
                write!(f, "] {} -> {}", Self::Tuple(sig.params.clone()), sig.result)
            }
        }
    }
}

/// Compile-time values as dumps show them: integers and types as written, variants by name.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => n.fmt(f),
            Self::Range(Some(range)) => write!(f, "{}..={}", Bound(range.lo()), Bound(range.hi())),
            Self::Range(None) => f.write_str("0..0"),
            Self::Type(ty) => ty.fmt(f),
            Self::Tuple(items) => {
                f.write_str("(")?;
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}{item}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(")")
            }
            Self::Array(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}{item}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str("]")
            }
            Self::Variant { name, .. } => write!(f, ".{name}"),
            Self::Struct { ty, .. } => write!(f, "{ty}{{ .. }}"),
            Self::Primitive(p) => write!(f, "{}.{}", p.namespace(), p.name()),
            Self::Namespace(ns) => {
                let names: Vec<_> = ns.path.iter().map(|n| n.as_str()).collect();
                f.write_str(&names.join("."))
            }
            Self::Fn(_) => f.write_str("fn"),
            Self::Closure { .. } => f.write_str("closure"),
            Self::Code(_) => f.write_str("#{ .. }"),
            Self::Linked(_) => f.write_str("linked"),
            Self::Interface(_) => f.write_str("interface"),
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
            arg.fmt(f)?;
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
