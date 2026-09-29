//! Patterns as coverage sees them: binders match anything, values are evaluated, and struct
//! fields take the order of the type's fields.

use vun_hir::tree::{Entry, ExprId, Name, PatId, PatKind};
use vun_sema_int::Interval;
use vun_sema_ty::{Ty, TyKind, Value};

use super::Pat;
use crate::member::Members;
use crate::typeck::Typer;

impl Typer<'_, '_, '_> {
    /// `pat` matched against values of type `ty`, if known.
    pub(super) fn lower(&self, pat: PatId, ty: Option<Ty>) -> Pat {
        let hir = self.hir();
        match &hir.pats[pat].kind {
            PatKind::Bind(_) | PatKind::Hole | PatKind::Rest(_) | PatKind::Computed(_) => Pat::Any,
            PatKind::At { pat, .. } => self.lower(*pat, ty),
            PatKind::Or(alternatives) => Pat::Or(alternatives.iter().map(|&p| self.lower(p, ty)).collect()),
            PatKind::Tuple(items) => {
                let parts = match ty.as_deref() {
                    Some(TyKind::Tuple(parts)) => parts.iter().copied().map(Some).collect(),
                    _ => Vec::new(),
                };
                let items: Vec<PatId> = items
                    .iter()
                    .filter_map(|e| if let Entry::One(p) = e { Some(*p) } else { None })
                    .collect();
                let mut lowered = Vec::with_capacity(parts.len());
                for (at, part) in parts.iter().enumerate() {
                    lowered.push(match items.get(at) {
                        // A rest pattern last matches the remaining items.
                        Some(&p)
                            if matches!(hir.pats[p].kind, PatKind::Rest(None)) && at + 1 == items.len() =>
                        {
                            lowered.resize(parts.len(), Pat::Any);
                            break;
                        }
                        Some(&p) => self.lower(p, *part),
                        None => Pat::Any,
                    });
                }
                Pat::Product(lowered)
            }
            PatKind::Variant { name: Name::Plain(name), args } => {
                let payload = self.payload_of(ty, *name);
                let args: Vec<PatId> = args
                    .iter()
                    .flatten()
                    .filter_map(|e| if let Entry::One(p) = e { Some(*p) } else { None })
                    .collect();
                let lowered = payload
                    .iter()
                    .enumerate()
                    .map(|(at, part)| args.get(at).map_or(Pat::Any, |&p| self.lower(p, *part)));
                Pat::Variant(*name, lowered.collect())
            }
            PatKind::Struct { fields, .. } => {
                let Some(names) = self.fields(ty) else { return Pat::Opaque };
                let mut lowered = vec![Pat::Any; names.len()];
                for (at, field) in fields.iter().enumerate() {
                    let Entry::One((name, p)) = field else { return Pat::Opaque };
                    let place = match name {
                        Some(Name::Plain(name)) => names.iter().position(|(n, _)| n == name),
                        None => Some(at),
                        Some(_) => None,
                    };
                    let Some(place) = place.filter(|&p| p < names.len()) else { return Pat::Opaque };
                    lowered[place] = self.lower(*p, names[place].1);
                }
                Pat::Product(lowered)
            }
            PatKind::Value(value) => self.constant(*value),
            PatKind::Range { lo, hi, inclusive } => {
                self.bounds(*lo, *hi, *inclusive).map_or(Pat::Opaque, Pat::Range)
            }
            PatKind::Variant { .. } | PatKind::Error => Pat::Opaque,
        }
    }

    /// The payload types of the variant `name` of `ty`; empty when unknown.
    pub(super) fn payload_of(&self, ty: Option<Ty>, name: vun_intern::Symbol) -> Vec<Option<Ty>> {
        let members = ty.and_then(|t| self.env().cx.get::<Members>(&t));
        let variant = members.as_deref().and_then(|m| m.variants.iter().find(|v| v.name == name));
        variant.map(|v| v.payload.iter().map(|p| p.as_ref().ok().copied()).collect()).unwrap_or_default()
    }

    /// The fields of a struct or record type, in its order, with their types where known.
    pub(super) fn fields(&self, ty: Option<Ty>) -> Option<Vec<(vun_intern::Symbol, Option<Ty>)>> {
        match ty.as_deref()? {
            TyKind::Record(fields) => Some(fields.iter().map(|(n, t)| (*n, Some(*t))).collect()),
            TyKind::Nominal(_) => {
                let members = self.env().cx.get::<Members>(&ty?)?;
                let complete = members.complete && members.variants.is_empty();
                complete
                    .then(|| members.fields.iter().map(|f| (f.name, f.ty.as_ref().ok().copied())).collect())
            }
            _ => None,
        }
    }

    /// A value pattern, by its value at compile time.
    fn constant(&self, value: ExprId) -> Pat {
        self.evaluating().eval(value).map_or(Pat::Opaque, |v| Self::valued(&v))
    }

    fn valued(value: &Value) -> Pat {
        match value {
            Value::Int(n) => Pat::Range(Interval::singleton(n.clone())),
            Value::Variant { name, payload, .. } => {
                Pat::Variant(*name, payload.iter().map(Self::valued).collect())
            }
            Value::Tuple(items) => Pat::Product(items.iter().map(Self::valued).collect()),
            Value::Struct { fields, .. } => {
                Pat::Product(fields.iter().map(|(_, v)| Self::valued(v)).collect())
            }
            _ => Pat::Opaque,
        }
    }

    /// `lo..hi`, `lo..=hi` or `lo..`, when its bounds are integers at compile time.
    fn bounds(&self, lo: ExprId, hi: Option<ExprId>, inclusive: bool) -> Option<Interval> {
        let int = |e| match self.evaluating().eval(e) {
            Ok(Value::Int(n)) => Some(n),
            _ => None,
        };
        let lo = int(lo)?;
        match hi {
            None => Some(Interval::at_least(lo)),
            Some(hi) if inclusive => Interval::closed(lo, int(hi)?),
            Some(hi) => Interval::half_open(lo, &int(hi)?),
        }
    }
}
