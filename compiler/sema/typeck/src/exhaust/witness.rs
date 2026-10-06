//! A value a `match` misses, written as a pattern would match it.

use std::fmt;

use vun_intern::Symbol;
use vun_sema_int::{Extended, Interval};

pub(super) enum Witness {
    Any,
    Variant(Symbol, Vec<Witness>),
    /// A tuple's items, or a struct's fields with their names.
    Product(Vec<Witness>, Option<Vec<Symbol>>),
    Range(Interval),
}

impl fmt::Display for Witness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.write_str("_"),
            Self::Variant(name, args) if args.is_empty() => write!(f, ".{name}"),
            Self::Variant(name, args) => write!(f, ".{name}({})", List(args)),
            Self::Product(items, None) if items.len() == 1 => write!(f, "({},)", items[0]),
            Self::Product(items, None) => write!(f, "({})", List(items)),
            Self::Product(fields, Some(names)) => {
                f.write_str(".{ ")?;
                for (i, (name, field)) in names.iter().zip(fields).enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, ".{name} = {field}")?;
                }
                f.write_str(" }")
            }
            Self::Range(range) => match (range.lo(), range.hi()) {
                (Extended::Finite(lo), Extended::Finite(hi)) if lo == hi => write!(f, "{lo}"),
                (Extended::Finite(lo), Extended::Finite(hi)) => write!(f, "{lo}..={hi}"),
                (Extended::Finite(lo), _) => write!(f, "{lo}.."),
                _ => f.write_str("_"),
            },
        }
    }
}

/// Patterns separated by commas.
struct List<'w>(&'w [Witness]);

impl fmt::Display for List<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, w) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{w}")?;
        }
        Ok(())
    }
}
