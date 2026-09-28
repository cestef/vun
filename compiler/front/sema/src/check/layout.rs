//! Layout attributes (spec 05, section 6): the fields of a `#[layout.bits(uN)]` struct take
//! exactly N bits, the tag of a `#[layout.tag(uN)]` enum numbers every variant,
//! `#[layout.align(N)]` aligns to a power of two, and `#[layout.offset(N)]` keeps fields in order.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::tree::{DeclId, ExprKind, Hir};
use vun_intern::Symbol;
use vun_origin::OriginId;

use super::Checker;
use crate::Primitive;
use crate::decl::DeclValue;
use crate::int::Integer;
use crate::item::DeclRef;
use crate::layout::Shapes;
use crate::member::{MemberTable, Members};
use crate::primitives::layout::Layout;
use crate::ty::{Ty, Value};

/// A `struct` or `enum` declaration being checked: its attributes' place, its type and members.
struct Laid<'a> {
    hir: &'a Hir,
    decl: DeclId,
    at: OriginId,
    members: &'a MemberTable,
}

impl Checker<'_, '_, '_> {
    /// Reports the layout attributes of the struct or enum declared by `decl` that its members
    /// break.
    pub(super) fn layout(&mut self, hir: &Hir, decl: DeclId) {
        let d = &hir.decls[decl];
        let Some(value) =
            d.value.filter(|&v| matches!(hir.exprs[v].kind, ExprKind::Struct(_) | ExprKind::Enum { .. }))
        else {
            return;
        };
        let cx = self.env.cx;
        let Ok(Value::Type(ty)) = cx.get::<DeclValue>(&DeclRef::new(self.env.file.clone(), hir, decl)) else {
            return;
        };
        let Some(members) = cx.get::<Members>(&ty) else { return };
        let laid = Laid { hir, decl, at: hir.exprs[value].origin, members: &members };
        if let Some([backing]) = self.args(&laid, Layout::bits).as_deref() {
            self.bits(&laid, backing.as_type());
        }
        if let Some([tag]) = self.args(&laid, Layout::tag).as_deref() {
            self.tag(&laid, tag.as_type());
        }
        if let Some([Value::Int(n)]) = self.args(&laid, Layout::align).as_deref() {
            self.align(&laid, n);
        }
        self.offsets(&laid);
    }

    fn args(&self, laid: &Laid<'_>, attr: Layout) -> Option<Vec<Value>> {
        self.env.attribute(&laid.hir.decls[laid.decl].attrs, Primitive::Layout(attr))
    }

    /// `#[layout.bits(uN)]`: the fields take N bits.
    fn bits(&mut self, laid: &Laid<'_>, backing: Option<Ty>) {
        let cx = self.env.cx;
        let Some(backing) = backing else { return };
        let fields = laid.members.fields.iter().map(|f| Shapes::bits(cx, *f.ty.as_ref().ok()?));
        let (Some(taken), Some(width)) = (fields.sum::<Option<u64>>(), Shapes::bits(cx, backing)) else {
            return;
        };
        if taken != width {
            let msg = Msg::new()
                .text("the fields take ")
                .num(i128::from(taken))
                .text(" bits, not the ")
                .num(i128::from(width))
                .text(" of ")
                .code(backing.to_string());
            self.diag.error(Diagnostic::error(errors::LAYOUT_BITS_WIDTH, msg).at(laid.at, None));
        }
    }

    /// `#[layout.tag(uN)]`: `uN` has a pattern for each variant.
    fn tag(&mut self, laid: &Laid<'_>, tag: Option<Ty>) {
        let Some(tag) = tag else { return };
        let variants = laid.members.variants.len();
        let holds = tag.interval().filter(|r| r.unsigned_width().is_some()).and_then(|r| r.count());
        if holds.is_some_and(|count| count >= Integer::from(variants)) {
            return;
        }
        let msg = Msg::new()
            .text("the tag type ")
            .code(tag.to_string())
            .text(" is not a ")
            .code("uN")
            .text(" type with a pattern for each of the ")
            .num(u64::try_from(variants).unwrap_or(u64::MAX))
            .text(" variants");
        self.diag.error(Diagnostic::error(errors::LAYOUT_TAG, msg).at(laid.at, None));
    }

    /// `#[layout.offset(N)]`: a field lies at or past the end of the one before it, as the
    /// compiler never reorders, and at a multiple of its alignment.
    fn offsets(&mut self, laid: &Laid<'_>) {
        let cx = self.env.cx;
        let mut end = 0u64;
        for field in &laid.members.fields {
            let Some(layout) = field.ty.as_ref().ok().and_then(|ty| Shapes::layout(cx, ty)) else { return };
            let at = match field.offset {
                Some(at) if at < end || at % layout.align != 0 => {
                    self.offset(laid, field.name, at, end, layout.align);
                    return;
                }
                Some(at) => at,
                None => end.next_multiple_of(layout.align),
            };
            end = at + layout.size;
        }
    }

    /// Reports the field `name` placed at `at`, before `end` or off its alignment `align`.
    fn offset(&mut self, laid: &Laid<'_>, name: Symbol, at: u64, end: u64, align: u64) {
        let msg = Msg::new().code(name.as_str()).text(" cannot be placed at ").num(at);
        let why = if at < end {
            Msg::new()
                .text("the fields before it end at ")
                .num(end)
                .text("; fields keep their declaration order")
        } else {
            Msg::new().text("its type is aligned to ").num(align).text(" bytes")
        };
        let ns = laid.hir.decls[laid.decl].value.and_then(|v| match laid.hir.exprs[v].kind {
            ExprKind::Struct(ns) => Some(ns),
            _ => None,
        });
        let at_field = ns.and_then(|ns| laid.hir.field(ns, name)).map_or(laid.at, |f| f.origin);
        let d = Diagnostic::error(errors::LAYOUT_OFFSET, msg).at(at_field, None).child(ChildKind::Note, why);
        self.diag.error(d);
    }

    /// `#[layout.align(N)]`: `N` is a power of two.
    fn align(&mut self, laid: &Laid<'_>, n: &Integer) {
        if n.to_u128().is_some_and(u128::is_power_of_two) {
            return;
        }
        let msg = Msg::new().text("the alignment ").code(n.to_string()).text(" is not a power of two");
        self.diag.error(Diagnostic::error(errors::LAYOUT_ALIGN, msg).at(laid.at, None));
    }
}
