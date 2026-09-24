//! The members of `struct` and `enum` types (spec 02, section 2): fields and declarations, which
//! make a struct's namespace, and variants. A struct built in a compile-time call has its members evaluated inside
//! that call, so `Pair(u8)` has fields of type `u8`.

use std::sync::Arc;

use overt_grammar::TokenKind;
use overt_hir::tree::{Entry, ExprKind, List, Member, Name, NsId, PatKind, Staged, Variant};
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use crate::decl::TypedDecl;
use crate::env::{Env, Unevaluated};
use crate::eval::Staging;
use crate::item::{DeclRef, ExprRef, Node};
use crate::ty::{Nominal, Ty, TyKind, Value};
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
    /// The value a literal that omits the field gives it, evaluated where the struct was built.
    pub default: Option<ExprRef>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MemberDecl {
    pub name: Symbol,
    pub decl: DeclRef,
    pub visible: Visible,
    /// The `$for` bindings it was generated under, which its value sees.
    pub staged: Vec<Staging>,
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
        let hir = &env.parsed.hir;
        for entry in entries {
            match entry {
                Entry::One(member) => self.member(env, member, home),
                Entry::Staged(Staged::If { cond, then, otherwise, .. }) => {
                    match env.eval(*cond).and_then(|c| env.holds(&c)) {
                        Ok(holds) => self.entries(env, if holds { then } else { otherwise }, home),
                        Err(_) => self.complete = false,
                    }
                }
                Entry::Staged(Staged::For { pat, iter, body, .. }) => {
                    let (Some(items), PatKind::Bind(binder)) =
                        (env.eval(*iter).ok().and_then(Value::items), &hir.pats[*pat].kind)
                    else {
                        self.complete = false;
                        continue;
                    };
                    let binder = Node { id: *binder, origin: hir.binders[*binder].origin };
                    for value in items {
                        let step = env.clone().staging(&[Staging { binder, value }]);
                        self.entries(&step, body, home);
                    }
                }
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
                let default = field.default.map(|d| ExprRef::new(env.file.clone(), hir, d));
                self.fields.push(Field { name, ty: env.ty(field.ty), visible, default });
            }
            Member::Decl(decl) => {
                let d = &hir.decls[*decl];
                let names = if let PatKind::Value(e) = &hir.pats[d.pat].kind {
                    // `$(e) := value`: a name computed at compile time.
                    match &hir.exprs[*e].kind {
                        ExprKind::Prefix { op: TokenKind::Dollar, operand, .. } => {
                            env.name(&Name::Computed(*operand)).into_iter().collect()
                        }
                        _ => Vec::new(),
                    }
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
            Member::Spread(_) => self.complete = false,
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
        let env = Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        env.staging(&decl.staged).decl_type(decl.decl.decl.id)
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

/// The value of the declaration `name` among a struct type's members, evaluated where the struct
/// was built: a method of `Pair(u8)` is a function value that sees `T` bound to `u8`.
pub struct MemberValue;

impl Query for MemberValue {
    const NAME: &'static str = "member_value";
    type Key = (Ty, Symbol);
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, (ty, name): &Self::Key) -> Self::Value {
        let TyKind::Nominal(Nominal { literal, instance, .. }) = &**ty else {
            return Err(Unevaluated::Unsupported);
        };
        let members = cx.get::<Members>(ty).ok_or(Unevaluated::Unsupported)?;
        let Found::Decl(decl) = members.find(*name) else { return Err(Unevaluated::Absent) };
        let types = cx.get::<TypedDecl>(&(decl.decl.clone(), instance.clone(), decl.staged.clone()));
        let env = Env::defining(cx, literal, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        let env = env.staging(&decl.staged).typed_by(types);
        let value = env.parsed.hir.decls[decl.decl.decl.id].value.ok_or(Unevaluated::Runtime)?;
        match env.parsed.hir.exprs[value].kind {
            // A requirement of an interface, which an impl provides.
            ExprKind::Hole => Err(Unevaluated::Provided),
            _ => env.eval(value),
        }
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
