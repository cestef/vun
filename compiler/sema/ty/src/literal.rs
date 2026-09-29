//! Compile-time values as literals (spec 01, section 7), which the literal writer prints: what
//! tools show of data that code computed, such as a package's plan.

use std::borrow::Cow;

use vun_hir::db::PackageId;
use vun_intern::Symbol;
use vun_span::Span;
use vun_syntax::literal::{Decl, Literal};

use super::{TyKind, Value};

impl Value {
    /// This value as a literal: integers, text (an array of bytes that is UTF-8), lists, records
    /// for `struct` values, `.name` and `.name(payload)` for variants and names, what an `Any`
    /// holds, namespaces by their path from the package, whose name `package` gives, and
    /// primitive types by their declaration's path, applied to their arguments as the call making
    /// them (`lang.Float(32)`); `None` for a value no literal spells, such as a function. `span` is where the
    /// fields it makes are said to be written.
    #[must_use]
    pub fn literal(
        &self,
        span: Span,
        package: &dyn Fn(PackageId) -> Option<Symbol>,
    ) -> Option<Literal<'static>> {
        let all = |items: &[Self]| items.iter().map(|v| v.literal(span, package)).collect::<Option<Vec<_>>>();
        Some(match self {
            Self::Int(n) => Literal::Int(n.to_i128()?),
            Self::Array(items) if !items.is_empty() && self.text().is_some() => {
                Literal::String(Cow::Owned(self.text()?))
            }
            Self::Array(items) | Self::Tuple(items) => Literal::List(all(items)?),
            Self::Struct { fields, .. } => Literal::Record(
                fields
                    .iter()
                    .map(|(name, v)| Some(Decl::new(name.as_str(), v.literal(span, package)?, span)))
                    .collect::<Option<_>>()?,
            ),
            Self::Variant { name, payload, .. } if payload.is_empty() => Literal::Name(name.as_str()),
            Self::Variant { name, payload, .. } => Literal::Tagged(name.as_str(), all(payload)?),
            Self::Name(name) => Literal::Name(name.as_str()),
            Self::Any { value, .. } => value.literal(span, package)?,
            Self::Namespace(ns) => {
                Literal::Path([package(ns.package)?].iter().chain(&ns.path).map(|n| n.as_str()).collect())
            }
            Self::Type(ty) => match &**ty {
                TyKind::Primitive(p, args) if args.is_empty() => Literal::Path(p.item().path().to_vec()),
                TyKind::Primitive(p, args) => Literal::Call(p.item().path().to_vec(), all(args)?),
                _ => return None,
            },
            _ => return None,
        })
    }
}
