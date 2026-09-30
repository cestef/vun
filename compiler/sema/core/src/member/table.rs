//! Reading a type's members from its literal: fields, declarations, variants, and what its
//! spreads bring.

use std::sync::Arc;

use vun_grammar::TokenKind;
use vun_hir::tree::{Entry, ExprKind, List, Member, Name, NsId, PatKind, Spread, Variant};
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::kind::Kind;
use vun_primitives::layout::Layout;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::item::{DeclRef, Declared, ExprRef};
use vun_sema_ty::{Nominal, Ty, TyKind};

use crate::env::Env;
use crate::visible::Home;

use super::{Field, MemberDecl, MemberTable, VariantDef};

/// The members of a `struct` or `enum` type; `None` for any other type.
pub struct Members;

impl Query for Members {
    const NAME: &'static str = "type_members";
    type Key = Ty;
    type Value = Option<Arc<MemberTable>>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else { return None };
        let env = Env::defining(cx, literal, instance.as_ref())?;
        let mut table = MemberTable { complete: true, ..MemberTable::default() };
        match &env.parsed.hir.exprs[literal.expr.id].kind {
            ExprKind::Struct(ns) => table.struct_members(&env, *ns, literal),
            ExprKind::Enum { variants, ns } => {
                table.variants(&env, variants);
                table.struct_members(&env, *ns, literal);
            }
            _ => return None,
        }
        Some(Arc::new(table))
    }

    /// A type's members read while they are being computed, as by `$for f in fields(Self)` in
    /// its own body (spec 03, section 6: the freeze rule).
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        None
    }
}

impl MemberTable {
    /// The fields and declarations of the body `ns` of the `struct` literal `literal`.
    fn struct_members(&mut self, env: &Env<'_, '_>, ns: NsId, literal: &ExprRef) {
        let home = Home::Struct(literal.clone());
        self.entries(env, &env.parsed.hir.namespaces[ns].members, &home);
    }

    /// The members `entries` give, generating those of `$for` and `$if` entries with their
    /// bindings (spec 03, section 2); what cannot be generated makes the table incomplete.
    fn entries(&mut self, env: &Env<'_, '_>, entries: &List<Member>, home: &Home) {
        for entry in entries {
            let mut members = Vec::new();
            if env.generated(entry, &mut members).is_err() {
                self.complete = false;
            }
            for (env, member) in members {
                self.member(&env, member, home);
            }
        }
    }

    /// One member written, or generated under the bindings of `env`.
    fn member(&mut self, env: &Env<'_, '_>, member: &Member, home: &Home) {
        let hir = &env.parsed.hir;
        match member {
            Member::Field(field) => {
                // A `_` field takes its space and is reached by no name, such as reserved bits.
                let name = match &field.name {
                    Name::Hole => TokenKind::Underscore.text().map(Symbol::intern),
                    name => env.name(name),
                };
                let Some(name) = name else {
                    self.complete = false;
                    return;
                };
                let (visible, _) = env.visible(name, &field.attrs, home.clone());
                if env.attribute(&field.attrs, Primitive::Kind(Kind::owns)).is_some() {
                    self.deref = Some(name);
                }
                let default = field.default.map(|d| ExprRef::new(env.file.clone(), hir, d));
                let offset = env.count(&field.attrs, Primitive::Layout(Layout::offset));
                let attrs = env.values(&field.attrs);
                self.fields.push(Field { name, ty: env.ty(field.ty), visible, default, offset, attrs });
            }
            Member::Decl(decl) => {
                let d = &hir.decls[*decl];
                // `$(e) := value`: a name computed at compile time.
                let names = if let PatKind::Computed(e) = &hir.pats[d.pat].kind {
                    env.name(&Name::Computed(*e)).into_iter().collect()
                } else {
                    let mut binders = Vec::new();
                    hir.binders(d.pat, &mut binders);
                    binders.into_iter().map(|b| hir.binders[b].name).collect::<Vec<_>>()
                };
                if names.is_empty() {
                    self.complete = false;
                }
                let decl = DeclRef::new(env.file.clone(), hir, *decl);
                for name in names {
                    let (visible, _) = env.visible(name, &d.attrs, home.clone());
                    let staged = env.staged().to_vec();
                    self.decls.push(MemberDecl { name, decl: decl.clone(), visible, staged });
                }
            }
            Member::Spread(spread) => self.spread(env, spread),
        }
    }

    /// The members `spread` brings, each where it is declared; one naming no namespace, which
    /// the name checks of its file report, leaves the table incomplete.
    fn spread(&mut self, env: &Env<'_, '_>, spread: &Spread) {
        let hir = &env.parsed.hir;
        let Some(target) = spread.target(env.cx, hir, env.file.package) else {
            self.complete = false;
            return;
        };
        for &name in spread.names(env.cx, hir, &target).iter() {
            match env.cx.get::<Declared>(&(target.clone(), name)) {
                Some(site) => self.spread.push((name, site)),
                None => self.complete = false,
            }
        }
    }

    /// The variants of an `enum` literal, in declaration order.
    fn variants(&mut self, env: &Env<'_, '_>, variants: &List<Variant>) {
        for entry in variants {
            match entry {
                Entry::One(Variant { name: Name::Plain(name), payload, attrs, .. }) => {
                    let payload = payload.iter().map(|&t| env.ty(t)).collect();
                    self.variants.push(VariantDef { name: *name, payload, attrs: env.values(attrs) });
                }
                _ => self.complete = false,
            }
        }
    }
}
