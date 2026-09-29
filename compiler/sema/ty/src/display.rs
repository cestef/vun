//! Types as diagnostics show them: integers by their range, primitives by name with their
//! arguments, literals by the declaration they are written in with its arguments, or by kind.

use std::fmt;

use vun_sema_int::{Extended, Integer};

use super::{Nominal, NominalKind, TyKind, Value};
use crate::ops::Endpoints;

impl fmt::Display for TyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(range) = self.interval() {
            // A range ending just below a power of two reads as the half-open range to it:
            // `Int(0..2^64)`.
            let past = range.hi().finite().map(|hi| hi + &Integer::from(1));
            return match (range.hi(), past) {
                (Extended::PosInf, _) => write!(f, "Int({}..)", Bound(range.lo())),
                (_, Some(past)) if Power::of(&past).is_some_and(|p| p.offset == 0) => {
                    write!(f, "Int({}..{})", Bound(range.lo()), Bound(&Extended::Finite(past)))
                }
                (hi, _) => write!(f, "Int({}..={})", Bound(range.lo()), Bound(hi)),
            };
        }
        if let Some(sig) = self.signature() {
            for marker in &sig.markers {
                write!(f, "{marker} ")?;
            }
            return write!(f, "{} -> {}", Self::Tuple(sig.params), sig.result);
        }
        // Borrows and sequences as written: `&mut T`, `[4]u8`, `[u8]`.
        if let Some((mode, _, ty)) = self.borrowing() {
            return if mode.is_plain() { write!(f, "&{ty}") } else { write!(f, "&{mode} {ty}") };
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
            // `.{ a: u8, b: .{ c: bool } }`: a record by its fields.
            Self::Record(fields) => {
                f.write_str(".{ ")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    write!(f, "{}{name}: {ty}", if i > 0 { ", " } else { "" })?;
                }
                f.write_str(" }")
            }
            Self::Param(param) => f.write_str(param.name.as_str()),
            Self::Frame(frame) => match frame.name {
                Some(name) => write!(f, "{name}.Frame"),
                None => f.write_str("Frame"),
            },
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
            Self::Float(x) => x.fmt(f),
            Self::Range(Some(range)) => write!(f, "{}..={}", Bound(range.lo()), Bound(range.hi())),
            Self::Range(None) => f.write_str("0..0"),
            Self::Bounds(Endpoints { start, end: Some(end) }) => write!(f, "{start}..{end}"),
            Self::Bounds(Endpoints { start, end: None }) => write!(f, "{start}.."),
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
            Self::Variant { name, .. } | Self::Name(name) => write!(f, ".{name}"),
            Self::Constructor { ty, name } => write!(f, "{ty}.{name}"),
            Self::Struct { ty, .. } => write!(f, "{ty}{{ .. }}"),
            Self::Primitive(p) => write!(f, "{}", p.item()),
            Self::Namespace(ns) => {
                let names: Vec<_> = ns.path.iter().map(|n| n.as_str()).collect();
                f.write_str(&names.join("."))
            }
            Self::Fn(_) => f.write_str("fn"),
            Self::Closure { .. } => f.write_str("closure"),
            Self::Code(_) => f.write_str("#{ .. }"),
            Self::Any { value, .. } => write!(f, "{value}"),
            Self::Origin(_) => f.write_str("origin"),
            Self::Linked(_) => f.write_str("linked"),
            Self::Frame(_) => f.write_str("frame"),
            Self::Interface(_) => f.write_str("interface"),
            Self::Loan { .. } => f.write_str("&mut _"),
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
            Extended::Finite(n) => match Power::of(n) {
                Some(power) => power.fmt(f),
                None => write!(f, "{n}"),
            },
            Extended::NegInf | Extended::PosInf => f.write_str("_"),
        }
    }
}

/// A large integer written by the power of two it lies near: `2^64 - 1`, `-2^31`.
struct Power {
    negative: bool,
    exponent: u64,
    /// What is added to `±2^exponent`, small.
    offset: i128,
}

impl Power {
    /// Integers of at most this many bits are written in decimal: up to `2^16`, so that `u16`
    /// reads `Int(0..=65535)`.
    const DECIMAL_BITS: u64 = 17;
    /// How far from the power an integer may lie and still be written by it.
    const NEAR: u128 = 16;

    fn of(n: &Integer) -> Option<Self> {
        let negative = n.is_negative();
        let magnitude = n.abs();
        if magnitude.bits() <= Self::DECIMAL_BITS {
            return None;
        }
        // The power just above the magnitude, or the one it is at or just past.
        [magnitude.bits(), magnitude.bits() - 1].into_iter().find_map(|exponent| {
            let power = Integer::power_of_two(u32::try_from(exponent).ok()?);
            let offset = (&magnitude - &power).to_i128().filter(|d| d.unsigned_abs() <= Self::NEAR)?;
            Some(Self { negative, exponent, offset: if negative { -offset } else { offset } })
        })
    }
}

impl fmt::Display for Power {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(f, "{sign}2^{}", self.exponent)?;
        match self.offset {
            0 => Ok(()),
            d if d > 0 => write!(f, " + {d}"),
            d => write!(f, " - {}", -d),
        }
    }
}
