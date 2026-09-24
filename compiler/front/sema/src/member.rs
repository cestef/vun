//! The members of `struct` and `enum` types (spec 02, section 2): fields and declarations, which
//! make a struct's namespace, and variants. A struct built in a compile-time call has its members evaluated inside
//! that call, so `Pair(u8)` has fields of type `u8`.

use std::sync::Arc;

use overt_hir::tree::{Entry, ExprKind, List, Member, Name, NsId, Variant};
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::env::{Env, Unevaluated};
use crate::item::{DeclRef, ExprRef};
use crate::ty::{Nominal, Ty, TyKind};
use crate::typeck::Known;
use crate::visible::{Home, Visible};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct MemberTable {
    /// Fields in declaration order.
    pub fields: Vec<Field>,
    pub decls: Vec<MemberDecl>,
    /// An enum's variants, in declaration order.
    pub variants: Vec<VariantDef>,
    /// Whether every member is written literally, with no `$for`, `$if`, computed name or
    /// spread: only then is a name missing from the table no member at all.
    pub complete: bool,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Field {
    pub name: Symbol,
    pub ty: Result<Ty, Unevaluated>,
    pub visible: Visible,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MemberDecl {
    pub name: Symbol,
    pub decl: DeclRef,
    pub visible: Visible,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct VariantDef {
    pub name: Symbol,
    pub payload: Vec<Result<Ty, Unevaluated>>,
}

/// What a name is among a type's members.
#[derive(Debug, PartialEq, Eq)]
pub enum Found<'m> {
    Field(&'m Field),
    Decl(&'m MemberDecl),
    Variant(&'m VariantDef),
    Absent,
    /// Not among the members written literally, but perhaps generated.
    Unknown,
}

impl MemberTable {
    #[must_use]
    pub fn find(&self, name: Symbol) -> Found<'_> {
        if let Some(field) = self.fields.iter().find(|f| f.name == name) {
            return Found::Field(field);
        }
        if let Some(decl) = self.decls.iter().find(|d| d.name == name) {
            return Found::Decl(decl);
        }
        if let Some(variant) = self.variants.iter().find(|v| v.name == name) {
            return Found::Variant(variant);
        }
        if self.complete { Found::Absent } else { Found::Unknown }
    }
}

/// The members of a `struct` or `enum` type; `None` for any other type.
pub struct Members;

impl Query for Members {
    const NAME: &'static str = "members";
    type Key = Ty;
    type Value = Option<Arc<MemberTable>>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else { return None };
        let env = Env::defining(cx, literal, instance.as_ref())?;
        let mut table = MemberTable { complete: true, ..MemberTable::default() };
        match &env.parsed.hir.exprs[literal.expr.id].kind {
            ExprKind::Struct(ns) => table.struct_members(&env, *ns, literal),
            ExprKind::Enum(variants) => table.variants(&env, variants),
            _ => return None,
        }
        Some(Arc::new(table))
    }
}

impl MemberTable {
    /// The fields and declarations of the body `ns` of the `struct` literal `literal`.
    fn struct_members(&mut self, env: &Env<'_, '_>, ns: NsId, literal: &ExprRef) {
        let hir = &env.parsed.hir;
        let home = Home::Struct(literal.clone());
        for entry in &hir.namespaces[ns].members {
            let Entry::One(member) = entry else {
                self.complete = false;
                continue;
            };
            match member {
                Member::Field(field) => match field.name {
                    Name::Plain(name) => {
                        let (visible, _) = env.visible(name, &field.attrs, home.clone());
                        self.fields.push(Field { name, ty: env.ty(field.ty), visible });
                    }
                    _ => self.complete = false,
                },
                Member::Decl(decl) => {
                    let d = &hir.decls[*decl];
                    let mut binders = Vec::new();
                    hir.binders(d.pat, &mut binders);
                    let decl = DeclRef::new(env.file.clone(), hir, *decl);
                    for b in binders {
                        let name = hir.binders[b].name;
                        let (visible, _) = env.visible(name, &d.attrs, home.clone());
                        self.decls.push(MemberDecl { name, decl: decl.clone(), visible });
                    }
                }
                Member::Spread(_) => self.complete = false,
            }
        }
    }

    /// The variants of an `enum` literal, in declaration order.
    fn variants(&mut self, env: &Env<'_, '_>, variants: &List<Variant>) {
        for entry in variants {
            match entry {
                Entry::One(Variant { name: Name::Plain(name), payload, .. }) => {
                    let payload = payload.iter().map(|&t| env.ty(t)).collect();
                    self.variants.push(VariantDef { name: *name, payload });
                }
                _ => self.complete = false,
            }
        }
    }
}

/// The type of the declaration `name` among a struct type's members, evaluated where the struct
/// was built: `Eq(u8).eq` is `(&u8, &u8) -> bool`.
pub struct MemberType;

impl Query for MemberType {
    const NAME: &'static str = "member_type";
    type Key = (Ty, Symbol);
    type Value = Result<Known, Unevaluated>;

    fn compute(cx: &Cx<'_>, (ty, name): &Self::Key) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else {
            return Err(Unevaluated::Unsupported);
        };
        let members = cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let Found::Decl(decl) = members.find(*name) else { return Err(Unevaluated::Absent) };
        Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?.decl_type(decl.decl.decl.id)
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
