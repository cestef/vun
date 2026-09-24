//! The members of `struct` types (spec 02, section 2): fields, and declarations, which make the
//! struct's namespace. A struct built in a compile-time call has its members evaluated inside
//! that call, so `Pair(u8)` has fields of type `u8`.

use std::sync::Arc;

use overt_hir::tree::{Entry, ExprKind, Member, Name};
use overt_intern::Symbol;
use overt_query::{Cx, Query};

use crate::env::{Env, Unevaluated};
use crate::item::DeclRef;
use crate::ty::{Nominal, NominalKind, Ty, TyKind};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct MemberTable {
    /// Fields in declaration order, with their types.
    pub fields: Vec<(Symbol, Result<Ty, Unevaluated>)>,
    pub decls: Vec<(Symbol, DeclRef)>,
    /// Whether every member is written literally, with no `$for`, `$if`, computed name or
    /// spread: only then is a name missing from the table no member at all.
    pub complete: bool,
}

/// What a name is among a type's members.
#[derive(Debug, PartialEq, Eq)]
pub enum Found<'m> {
    Field(&'m Result<Ty, Unevaluated>),
    Decl(&'m DeclRef),
    Absent,
    /// Not among the members written literally, but perhaps generated.
    Unknown,
}

impl MemberTable {
    #[must_use]
    pub fn find(&self, name: Symbol) -> Found<'_> {
        if let Some((_, ty)) = self.fields.iter().find(|(n, _)| *n == name) {
            return Found::Field(ty);
        }
        if let Some((_, decl)) = self.decls.iter().find(|(n, _)| *n == name) {
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
        let env = match instance {
            Some(call) => Env::within(cx, call)?,
            None => Env::of(cx, &literal.file)?,
        };
        let hir = &env.parsed.hir;
        let ExprKind::Struct(ns) = hir.exprs[literal.expr].kind else { return None };
        let mut table = MemberTable { complete: true, ..MemberTable::default() };
        for entry in &hir.namespaces[ns].members {
            let Entry::One(member) = entry else {
                table.complete = false;
                continue;
            };
            match member {
                Member::Field(field) => match field.name {
                    Name::Plain(name) => table.fields.push((name, env.ty(field.ty))),
                    _ => table.complete = false,
                },
                Member::Decl(decl) => {
                    let mut binders = Vec::new();
                    hir.binders(hir.decls[*decl].pat, &mut binders);
                    let decl = DeclRef { file: env.file.clone(), decl: *decl };
                    table.decls.extend(binders.iter().map(|&b| (hir.binders[b].name, decl.clone())));
                }
                Member::Spread(_) => table.complete = false,
            }
        }
        Some(Arc::new(table))
    }
}
