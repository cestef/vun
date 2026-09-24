//! Records (spec 02, section 2): an anonymous literal written where no type is expected has a
//! structural type from what it holds, `.{ .a = 1 }` a record and `.{ 1, 2 }` a tuple; a bare
//! `.name` there is a `lang.meta.Name`.

use overt_hir::tree::{Arg, Entry, ExprId, List, Name};
use overt_hir::visit;
use overt_intern::Symbol;

use super::{Known, Stage, Typer};
use crate::Primitive;
use crate::primitives::meta::Meta;
use crate::ty::{Ty, TyKind};

impl Typer<'_, '_, '_> {
    /// `.{ ... }` with no type expected: a record of its named fields, or a tuple of its
    /// positional items; `None` for a mix, or a part whose type is not known.
    pub(super) fn record_lit(&mut self, fields: &[Entry<Arg>]) -> Option<Known> {
        let mut named: Vec<(Symbol, Ty)> = Vec::new();
        let mut items: Vec<Ty> = Vec::new();
        let mut stage = Stage::Compile;
        let mut known = true;
        for field in fields {
            let Entry::One(Arg { name, value }) = field else { return None };
            let found = self.typed(*value);
            stage = stage.max(found.map_or(self.stage, |k| k.stage));
            match (name, found) {
                (Some(Name::Plain(name)), Some(found)) => named.push((*name, found.ty)),
                (None, Some(found)) => items.push(found.ty),
                _ => known = false,
            }
        }
        let ty = match (named.is_empty(), items.is_empty()) {
            (_, true) if known => {
                named.sort_by_key(|(name, _)| name.as_str());
                TyKind::Record(named).intern()
            }
            (true, false) if known => TyKind::Tuple(items).intern(),
            _ => return None,
        };
        Some(Known { ty, stage })
    }

    /// A bare `.name` with no type expected: the name itself.
    pub(super) fn bare_name() -> Known {
        Known { ty: TyKind::primitive(Primitive::Meta(Meta::Name)), stage: Stage::Compile }
    }

    /// The field `name` of a value of the record type `ty`, reporting one it lacks.
    pub(super) fn record_field(&mut self, expr: ExprId, ty: Ty, name: Symbol, stage: Stage) -> Option<Known> {
        let TyKind::Record(fields) = &*ty else { return None };
        let found = fields.iter().find(|(n, _)| *n == name).map(|(_, field)| Known { ty: *field, stage });
        if found.is_none() {
            let at = self.hir().exprs[expr].origin;
            self.no_member(at, ty, name);
        }
        found
    }

    /// `.name(args)`: typed by its context, as a variant's constructor; without one it has no
    /// type, and its arguments are typed alone.
    pub(super) fn dot_call(&mut self, expr: ExprId, callee: ExprId, args: &List<Arg>) -> Option<Known> {
        let Some(expected) = self.expected(expr) else {
            let hir = self.hir();
            visit::walk_list(self, hir, args, &mut |v, a: &Arg| {
                v.typed(a.value);
            });
            return None;
        };
        self.expected.insert(callee, expected);
        self.call(expr, callee, args)
    }
}
