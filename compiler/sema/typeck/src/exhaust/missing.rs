//! Usefulness: a vector of values no row of a matrix of patterns covers, found column by column.
//! A column whose patterns split its type (variants, parts, ranges) is tried constructor by
//! constructor; any other column is covered by the rows that match anything in it.

use vun_intern::Symbol;
use vun_sema_int::{Extended, Integer, Interval};
use vun_sema_ty::{Ty, TyKind};

use super::Pat;
use super::witness::Witness;
use crate::Typer;
use vun_sema::member::Members;

/// How the values of a column's type split.
enum Shape {
    /// Each variant with its payload's types.
    Enum(Vec<(Symbol, Vec<Option<Ty>>)>),
    /// One constructor: a tuple's items, or a struct's fields by name.
    Product(Vec<Option<Ty>>, Option<Vec<Symbol>>),
    Int(Interval),
    /// Values no pattern but a wildcard covers.
    Whole,
    /// Not known here: counted as covered.
    Unknown,
}

impl Typer<'_, '_, '_> {
    /// Values of types `tys` that no row of `rows` covers, one per column, if there are any.
    pub(super) fn missing(&self, rows: Vec<Vec<Pat>>, tys: &[Option<Ty>]) -> Option<Vec<Witness>> {
        let rows = Self::expanded(rows);
        let Some((&ty, rest)) = tys.split_first() else { return rows.is_empty().then(Vec::new) };
        let split = rows.iter().any(|r| !matches!(r[0], Pat::Any | Pat::Opaque));
        match if split { self.shape(ty) } else { Shape::Whole } {
            Shape::Enum(variants) => variants.into_iter().find_map(|(name, payload)| {
                let arity = payload.len();
                let rows = Self::specialized(&rows, arity, |head| match head {
                    Pat::Variant(n, args) if *n == name => Some(args.clone()),
                    _ => None,
                });
                let witness = self.missing(rows, &[payload, rest.to_vec()].concat())?;
                Some(Self::rebuilt(witness, arity, |args| Witness::Variant(name, args)))
            }),
            Shape::Product(parts, names) => {
                let arity = parts.len();
                let rows = Self::specialized(&rows, arity, |head| match head {
                    Pat::Product(items) => Some(items.clone()),
                    _ => None,
                });
                let witness = self.missing(rows, &[parts, rest.to_vec()].concat())?;
                Some(Self::rebuilt(witness, arity, |items| Witness::Product(items, names)))
            }
            Shape::Int(domain) => Self::segments(&domain, &rows).into_iter().find_map(|segment| {
                let rows = Self::specialized(&rows, 0, |head| match head {
                    Pat::Range(range) if range.contains(&segment) => Some(Vec::new()),
                    _ => None,
                });
                let witness = self.missing(rows, rest)?;
                Some(Self::rebuilt(witness, 0, |_| Witness::Range(segment)))
            }),
            Shape::Whole => {
                let rows = Self::specialized(&rows, 0, |_| None);
                let witness = self.missing(rows, rest)?;
                Some(Self::rebuilt(witness, 0, |_| Witness::Any))
            }
            Shape::Unknown => {
                let rows = rows.into_iter().map(|r| r[1..].to_vec()).collect();
                let witness = self.missing(rows, rest)?;
                Some(Self::rebuilt(witness, 0, |_| Witness::Any))
            }
        }
    }

    /// Rows whose first pattern is an alternative, one per alternative.
    fn expanded(rows: Vec<Vec<Pat>>) -> Vec<Vec<Pat>> {
        let mut out = Vec::with_capacity(rows.len());
        let mut pending = rows;
        pending.reverse();
        while let Some(row) = pending.pop() {
            match row.first() {
                Some(Pat::Or(alternatives)) => {
                    for alternative in alternatives.iter().rev() {
                        let mut r = row.clone();
                        r[0] = alternative.clone();
                        pending.push(r);
                    }
                }
                _ => out.push(row),
            }
        }
        out
    }

    /// The rows matching a constructor of `arity` parts, which `parts` gives for a first pattern
    /// of that constructor: those parts, or wildcards for a first pattern that matches anything,
    /// then the rest of the row.
    fn specialized(
        rows: &[Vec<Pat>],
        arity: usize,
        parts: impl Fn(&Pat) -> Option<Vec<Pat>>,
    ) -> Vec<Vec<Pat>> {
        let heads = rows.iter().filter_map(|row| {
            let head = match &row[0] {
                Pat::Any => vec![Pat::Any; arity],
                // A head of another arity, of a pattern that does not type, matches nothing here.
                head => parts(head).filter(|parts| parts.len() == arity)?,
            };
            Some([head, row[1..].to_vec()].concat())
        });
        heads.collect()
    }

    /// The first `arity` values of `witness` made into one by `make`, then the rest.
    fn rebuilt(
        witness: Vec<Witness>,
        arity: usize,
        make: impl FnOnce(Vec<Witness>) -> Witness,
    ) -> Vec<Witness> {
        let mut witness = witness;
        let rest = witness.split_off(arity);
        let mut rebuilt = vec![make(witness)];
        rebuilt.extend(rest);
        rebuilt
    }

    /// `domain` cut at every bound of the ranges first in `rows`, so that each piece is inside or
    /// outside each range.
    fn segments(domain: &Interval, rows: &[Vec<Pat>]) -> Vec<Interval> {
        let one = Extended::Finite(Integer::from(1));
        let mut cuts: Vec<Extended> = vec![domain.lo().clone(), domain.hi().add(&one)];
        for row in rows {
            if let Pat::Range(range) = &row[0] {
                cuts.push(range.lo().clone());
                cuts.push(range.hi().add(&one));
            }
        }
        cuts.retain(|c| c >= domain.lo() && *c <= domain.hi().add(&one));
        cuts.sort();
        cuts.dedup();
        let pieces = cuts.windows(2).filter_map(|w| Interval::new(w[0].clone(), w[1].add(&one.neg())));
        pieces.collect()
    }

    fn shape(&self, ty: Option<Ty>) -> Shape {
        let Some(ty) = ty else { return Shape::Unknown };
        if let Some(domain) = ty.interval() {
            return Shape::Int(domain);
        }
        match &*ty {
            TyKind::Tuple(parts) => Shape::Product(parts.iter().copied().map(Some).collect(), None),
            TyKind::Record(_) | TyKind::Nominal(_) => {
                let Some(members) = self.env().cx.get::<Members>(&ty) else { return Shape::Unknown };
                if !members.complete {
                    return Shape::Unknown;
                }
                if members.variants.is_empty() {
                    let fields = self.fields(Some(ty)).unwrap_or_default();
                    let names = fields.iter().map(|(n, _)| *n).collect();
                    return Shape::Product(fields.into_iter().map(|(_, t)| t).collect(), Some(names));
                }
                let variants = members.variants.iter().map(|v| (v.name, self.payload_of(Some(ty), v.name)));
                Shape::Enum(variants.collect())
            }
            _ => Shape::Whole,
        }
    }
}
