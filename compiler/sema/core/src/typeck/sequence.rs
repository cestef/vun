//! Sequences: tuples, arrays and indexing (spec 02, section 2). Items are typed as the expected
//! items; an array literal has the join of its items (`[1, 2, 3]` is `[3]Int(1..=3)`, widened on
//! use), and indexing an array or a slice, directly or through a borrow, gives its item.

use vun_diag::{Diagnostic, Fix, Msg, Safety, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::{Entry, ExprId, ExprKind};
use vun_origin::OriginTable;
use vun_sema_int::Integer;
use vun_sema_ty::{Ty, TyKind, Value};
use vun_span::Span;

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// `(a, b)`: a tuple of its items' types, each typed as and checked against the expected
    /// tuple's item; or a tuple type, of types.
    pub(super) fn tuple(&mut self, expr: ExprId, items: &[Entry<ExprId>]) -> Option<Known> {
        // Where a type is expected, a tuple of types is a type: `()` is the unit type.
        let types = TyKind::of_types();
        if self.expected(expr) == Some(types) {
            let mut fit = true;
            let decided = self.each_item(items, &mut |t, item| match t.typed_as(item, Some(types)) {
                Some(known) => t.expect(t.hir().exprs[item].origin, known, types, None),
                None => fit = false,
            });
            return (decided && fit).then_some(Known { ty: types, stage: Stage::Compile });
        }
        // The expected tuple's items are matched by position when every item is written.
        let written = items.iter().all(|i| matches!(i, Entry::One(_)));
        let expected = self.expected(expr).and_then(|t| match &*t {
            TyKind::Tuple(parts) if written && parts.len() == items.len() => Some(parts.clone()),
            _ => None,
        });
        let mut parts = Vec::new();
        let mut stage = Stage::Compile;
        let decided = self.each_item(items, &mut |t, item| {
            let part = expected.as_ref().map(|p| p[parts.len()]);
            let known = t.typed_as(item, part);
            if let Some(known) = known {
                stage = stage.max(known.stage);
            }
            // An item that does not fit is reported on its own, and the tuple has no type.
            if let (Some(known), Some(part)) = (known, part)
                && !part.accepts(&known.ty)
            {
                t.expect(t.hir().exprs[item].origin, known, part, None);
                parts.push(None);
                return;
            }
            parts.push(known.map(|k| k.ty));
        });
        if !decided {
            return None;
        }
        let parts = parts.into_iter().collect::<Option<_>>()?;
        Some(Known { ty: TyKind::Tuple(parts).intern(), stage })
    }

    /// `[a, b, c]`: an array of the join of its items, each typed as the expected item.
    pub(super) fn array(&mut self, expr: ExprId, items: &[Entry<ExprId>]) -> Option<Known> {
        let expected = self.expected(expr).and_then(|t| t.items());
        let mut joined: Option<Option<Known>> = None;
        let mut len = 0_usize;
        let decided = self.each_item(items, &mut |t, item| {
            len += 1;
            let found = t.typed_as(item, expected);
            // An item that does not fit the expected item is reported on its own.
            if let (Some(found), Some(expected)) = (found, expected)
                && !expected.accepts(&found.ty)
            {
                t.expect(t.hir().exprs[item].origin, found, expected, None);
                joined = Some(None);
                return;
            }
            joined = Some(match (joined.take(), found) {
                (None, found) => found,
                (Some(Some(sofar)), Some(found)) => t.join(item, sofar, found),
                _ => None,
            });
        });
        if !decided {
            return None;
        }
        let len = Integer::from(i128::try_from(len).ok()?);
        match joined {
            // `[]` has the expected item type, if any.
            None => Some(Known { ty: TyKind::array(len, expected?), stage: Stage::Compile }),
            Some(joined) => {
                let joined = joined?;
                Some(Known { ty: TyKind::array(len, joined.ty), stage: joined.stage })
            }
        }
    }

    /// Runs `each` on every item of `items`, those `$for` and `$if` generate included, each in
    /// its step (spec 03, section 2); `false` when a `$for` or `$if` not decided here generates
    /// some, whose number is then not known.
    fn each_item(&mut self, items: &[Entry<ExprId>], each: &mut dyn FnMut(&mut Self, ExprId)) -> bool {
        let hir = self.hir();
        let outer = self.undecided;
        let mut decided = true;
        for item in items {
            match item {
                Entry::One(item) => each(self, *item),
                Entry::Staged(staged) => self.unroll(hir, staged, &mut |t, item| {
                    decided &= t.undecided == outer;
                    each(t, *item);
                }),
            }
        }
        decided
    }

    /// `[value; count]`: `count` copies of `value`, `count` known at compile time.
    pub(super) fn repeat(&mut self, expr: ExprId, value: ExprId, count: ExprId) -> Option<Known> {
        let expected = self.expected(expr).and_then(|t| t.as_array().map(|(_, elem)| elem));
        let item = self.typed_as(value, expected);
        self.typed(count);
        let len = match self.evaluating().eval(count) {
            Ok(Value::Int(len)) => len,
            Ok(_) => return None,
            Err(why) => {
                self.needed_when_compiling(count, &why);
                return None;
            }
        };
        let item = item?;
        Some(Known { ty: TyKind::array(len, item.ty), stage: item.stage })
    }

    /// `object[index]`, looking through a borrow: with an integer index, the item of an array or
    /// a slice; with bounds, the part they cover, a slice reached as the object is.
    pub(super) fn index(&mut self, object: ExprId, index: ExprId) -> Option<Known> {
        let object = self.typed(object);
        let written = self.typed(index)?;
        let object = object?;
        let stage = object.stage.max(written.stage);
        let (borrow, ty) = match object.ty.borrow() {
            Some((borrow, ty)) => (Some(borrow), ty),
            None => (None, object.ty),
        };
        let elem = ty.items()?;
        if written.ty.interval().is_some() {
            return Some(Known { ty: elem, stage });
        }
        if written.ty.bounds().is_none() {
            self.bad_index(index, written.ty);
            return None;
        }
        let part = TyKind::slice(elem);
        let ty = match borrow {
            Some(borrow) => TyKind::Primitive(borrow, vec![Value::Type(part)]).intern(),
            None => part,
        };
        Some(Known { ty, stage })
    }

    /// Reports `index`, of type `ty`, indexing a sequence though it is neither an integer nor
    /// bounds. `..n`, which Rust writes for the first `n` items, is a pack here: the help writes
    /// its start.
    fn bad_index(&mut self, index: ExprId, ty: Ty) {
        let hir = self.hir();
        let origin = hir.exprs[index].origin;
        let msg = Msg::new().text("cannot index with ").ty(ty.to_string());
        let d = Diagnostic::error(errors::TYPES_BAD_INDEX, msg)
            .at(origin, Some(Msg::new().text("this is ").ty(ty.to_string())))
            .help("index with an integer, or with bounds such as `a..b`, `a..=b` or `a..`");
        let d = match hir.exprs[index].kind {
            ExprKind::Prefix { op: TokenKind::DotDot, .. } => {
                let span = OriginTable::global().source_span(origin);
                let at = Span::new(span.file, span.range.lo, span.range.lo);
                d.fix(
                    Fix::replace(Msg::new().text("start the range at ").code("0"), Safety::Safe, at, "0")
                        .preferred(),
                )
            }
            _ => d,
        };
        self.diag.error(d);
    }

    /// An entry generated by `$for` or `$if`, whose items are typed without a known count.
    /// Types the values of `entry` when it is staged (`$for`, `$if`), each item being the
    /// expression `value` gives of it, so that what is wrong inside one is reported.
    pub(super) fn walk_entry<T>(&mut self, entry: &Entry<T>, value: fn(&T) -> ExprId) {
        let hir = self.hir();
        if let Entry::Staged(staged) = entry {
            vun_hir::visit::Visitor::staged(self, hir, staged, &mut |v, item: &T| {
                v.typed(value(item));
            });
        }
    }
}
