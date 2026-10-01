//! The requirement a function literal implements (spec 02, section 3): a member of the struct a
//! witness is, which an impl registers with `lang.impl(I(T))` on the declaration of the struct or
//! of the function making it for the parameters of its target, or which an interface derives with
//! `#[lang.derive(f)]`, `f` building the struct for the type it is given.

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Param, PatKind};
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Nominal, TyKind, Value};

use super::Env;
use crate::callable::Written;
use crate::member::{Found, Members};
use crate::witness::Derives;

/// A function literal that is a member of a witness: the name it is declared by, and what the
/// witness is registered for.
struct Witnessed {
    name: Symbol,
    /// The interface.
    interface: Source,
    /// The type the witness is for.
    target: Target,
    /// The function making the witness, where it has parameters.
    maker: Option<ExprId>,
}

/// Where the interface of a witness is written, or known.
enum Source {
    /// An expression of the impl attribute.
    Expr(ExprId),
    /// The interface that derives the witness.
    Derived(Value),
}

/// The type a witness is for.
enum Target {
    /// An expression of the impl attribute: a type, or the constructor the witness's function
    /// applies to its parameters.
    Expr(ExprId),
    /// The type the function deriving the witness is given.
    Given,
}

impl<'a, 'db> Env<'a, 'db> {
    /// Where the interface writes the type of the requirement the function literal `literal`
    /// implements, if it is a member of a witness: why it is not known here, when the requirement
    /// names what the impl binds and no instantiation does.
    #[must_use]
    pub fn asked(&self, literal: ExprId) -> Option<Result<Written<'a, 'db>, Unevaluated>> {
        let witnessed = self.witnessed(literal)?;
        Some(self.requirement(&witnessed))
    }

    /// The witness `literal` is a member of, if it is.
    fn witnessed(&self, literal: ExprId) -> Option<Witnessed> {
        let hir = &self.parsed.hir;
        let decl = self.declaration_of(literal).0?;
        let PatKind::Bind(binder) = hir.pats[hir.decls[decl].pat].kind else { return None };
        let name = hir.binders[binder].name;
        let witness = hir.struct_of(hir.member_ns(binder)?)?;
        if let Some(declaration) = self.declaration_of(witness).0 {
            let (interface, target) = self.registration(declaration)?;
            return Some(Witnessed { name, interface, target, maker: None });
        }
        let maker = hir.enclosing_function(witness)?;
        if let Some(declaration) = self.declaration_of(maker).0
            && let Some((interface, target)) = self.registration(declaration)
        {
            return Some(Witnessed { name, interface, target, maker: Some(maker) });
        }
        let interface = self.deriving(maker)?;
        Some(Witnessed {
            name,
            interface: Source::Derived(interface),
            target: Target::Given,
            maker: Some(maker),
        })
    }

    /// The interface and target `declaration` registers a witness for with `lang.impl(I(T))`.
    fn registration(&self, declaration: vun_hir::tree::DeclId) -> Option<(Source, Target)> {
        let hir = &self.parsed.hir;
        hir.decls[declaration].attrs.iter().find_map(|&attr| {
            let ExprKind::Call { callee, args } = &hir.exprs[attr].kind else { return None };
            let Ok(Value::Primitive(Primitive::Lang(Lang::r#impl))) = self.eval(*callee) else { return None };
            let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
            let ExprKind::Call { callee: interface, args } = &hir.exprs[*value].kind else { return None };
            let [Entry::One(Arg { name: None, value: target })] = args.as_slice() else { return None };
            matches!(self.eval(*interface), Ok(Value::Interface(_)))
                .then_some((Source::Expr(*interface), Target::Expr(*target)))
        })
    }

    /// The interface `maker`, a function of this file, derives witnesses for: the one marked
    /// `#[lang.derive(maker)]`.
    fn deriving(&self, maker: ExprId) -> Option<Value> {
        let derives = self.cx.get::<Derives>(&self.file);
        derives
            .iter()
            .find(|d| matches!(&d.with, Value::Fn(f) if f.literal.file == self.file && f.literal.expr.id == maker))
            .map(|d| Value::Interface(d.interface.clone()))
    }

    /// The function type the interface of `witnessed` writes for the requirement of the member's
    /// name, in the environment it is evaluated in.
    fn requirement(&self, witnessed: &Witnessed) -> Result<Written<'a, 'db>, Unevaluated> {
        let hir = &self.parsed.hir;
        let interface = match &witnessed.interface {
            Source::Expr(interface) => self.eval(*interface)?,
            Source::Derived(interface) => interface.clone(),
        };
        // The parameters of the function making the witness, bound here.
        let given = |maker| -> Result<Vec<Value>, Unevaluated> {
            let ExprKind::Fn { params, .. } = &hir.exprs[maker].kind else { return Err(Unevaluated::Absent) };
            params
                .iter()
                .map(|p| match p {
                    Entry::One(Param { pat: Some(pat), .. }) => match hir.pats[*pat].kind {
                        PatKind::Bind(b) => self.bound(b).cloned().ok_or(Unevaluated::Unbound),
                        _ => Err(Unevaluated::Unsupported),
                    },
                    _ => Err(Unevaluated::Unsupported),
                })
                .collect()
        };
        // The type the witness is for: the target, or what it makes of the parameters the
        // witness's function binds, or the type that function is given.
        let target = match (&witnessed.target, witnessed.maker) {
            (Target::Expr(target), None) => self.eval(*target)?,
            (Target::Expr(target), Some(maker)) => self.apply(self.eval(*target)?, given(maker)?)?,
            (Target::Given, Some(maker)) => given(maker)?.into_iter().next().ok_or(Unevaluated::Absent)?,
            (Target::Given, None) => return Err(Unevaluated::Absent),
        };
        let requirements = self.apply(interface, vec![target])?.as_type().ok_or(Unevaluated::Unsupported)?;
        let TyKind::Nominal(Nominal { literal: made, instance, .. }) = &*requirements else {
            return Err(Unevaluated::Unsupported);
        };
        let members = self.cx.get::<Members>(&requirements).ok_or(Unevaluated::Unsupported)?;
        let Found::Decl(required) = members.find(witnessed.name) else { return Err(Unevaluated::Absent) };
        let env = Self::defining(self.cx, made, instance.as_ref()).ok_or(Unevaluated::Absent)?;
        let env = env.staging(&required.staged);
        let their_hir = &env.parsed.hir;
        let mut expr = their_hir.decls[required.decl.decl.id].ty.ok_or(Unevaluated::Unsupported)?;
        while let ExprKind::Prefix { operand, .. } = their_hir.exprs[expr].kind {
            expr = operand;
        }
        match their_hir.exprs[expr].kind {
            ExprKind::FnType { .. } => Ok(Written { env, expr }),
            _ => Err(Unevaluated::Unsupported),
        }
    }
}
