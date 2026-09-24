//! The members of `struct` types (spec 02, section 2): fields, and declarations, which make the
//! struct's namespace. A struct built in a compile-time call has its members evaluated inside
//! that call, so `Pair(u8)` has fields of type `u8`.

use std::sync::Arc;

use overt_hir::tree::{Entry, ExprKind, Member, Name};
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::env::{Env, Unevaluated};
use crate::item::DeclRef;
use crate::ty::{Nominal, NominalKind, Ty, TyKind};
use crate::typeck::Known;
use crate::visible::{Home, Visible};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct MemberTable {
    /// Fields in declaration order.
    pub fields: Vec<Field>,
    pub decls: Vec<MemberDecl>,
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

/// What a name is among a type's members.
#[derive(Debug, PartialEq, Eq)]
pub enum Found<'m> {
    Field(&'m Field),
    Decl(&'m MemberDecl),
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
        if self.complete { Found::Absent } else { Found::Unknown }
    }
}

/// The members of a `struct` type; `None` for any other type.
pub struct Members;

impl Query for Members {
    const NAME: &'static str = "members";
    type Key = Ty;
    type Value = Option<Arc<MemberTable>>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, kind: NominalKind::Struct, .. }) = &**ty else {
            return None;
        };
        let env = Env::defining(cx, literal, instance.as_ref())?;
        let hir = &env.parsed.hir;
        let ExprKind::Struct(ns) = hir.exprs[literal.expr].kind else { return None };
        let mut table = MemberTable { complete: true, ..MemberTable::default() };
        for entry in &hir.namespaces[ns].members {
            let Entry::One(member) = entry else {
                table.complete = false;
                continue;
            };
            let home = Home::Struct(literal.clone());
            match member {
                Member::Field(field) => match field.name {
                    Name::Plain(name) => {
                        let (visible, _) = env.visible(name, &field.attrs, home);
                        table.fields.push(Field { name, ty: env.ty(field.ty), visible });
                    }
                    _ => table.complete = false,
                },
                Member::Decl(decl) => {
                    let d = &hir.decls[*decl];
                    let mut binders = Vec::new();
                    hir.binders(d.pat, &mut binders);
                    let decl = DeclRef { file: env.file.clone(), decl: *decl };
                    for b in binders {
                        let name = hir.binders[b].name;
                        let (visible, _) = env.visible(name, &d.attrs, home.clone());
                        table.decls.push(MemberDecl { name, decl: decl.clone(), visible });
                    }
                }
                Member::Spread(_) => table.complete = false,
            }
        }
        Some(Arc::new(table))
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
        Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?.decl_type(decl.decl.decl)
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
