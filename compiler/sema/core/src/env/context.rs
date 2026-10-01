//! What the place of a function literal says of its signature (spec 02, section 3), for what the
//! literal leaves out: the type of a parameter written without one, and a result not written.
//! The place is the declaration it is the value of, annotated with a function type (`add: (a:
//! u8, b: u8) -> u8 = (a, b) { ... }`), or the interface a witness it is a member of implements,
//! whose requirement of the same name is `format := ($W: Writer, value, w) { ... }`'s, with the
//! `Self` the impl is for and the `$` parameters the member binds. Where a member leaves its
//! parameters' types out, the requirement is where they are written (`Func::written`), so calls
//! infer its `$` parameters and pass its arguments as the requirement says.

use std::sync::Arc;

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Param, PatKind};
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Nominal, Signature, Ty, TyKind, Value};

use super::Env;
use crate::callable::Written;
use crate::member::{Found, Members};

/// The types a place gives the parts of a function literal; why for a part it cannot give here,
/// such as a result that names a member of a `$` parameter an instantiation binds.
pub struct Context {
    pub params: Vec<Result<Ty, Unevaluated>>,
    pub result: Result<Ty, Unevaluated>,
    pub place: Place,
}

/// What gives a function literal its context.
#[derive(Clone, Copy)]
pub enum Place {
    /// The function type its declaration is annotated with.
    Annotation,
    /// The requirement of an interface it implements.
    Requirement,
}

impl Place {
    /// How a report names it.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Annotation => "the function type it is annotated with",
            Self::Requirement => "the requirement it implements",
        }
    }
}

impl Context {
    /// The type of the parameter at `index`, or why it is not known here; none if the place has
    /// no parameter there.
    #[must_use]
    pub fn param(&self, index: usize) -> Option<Result<Ty, Unevaluated>> {
        self.params.get(index).cloned()
    }
}

impl From<Signature> for Context {
    fn from(signature: Signature) -> Self {
        let params = signature.params.into_iter().map(Ok).collect();
        Self { params, result: Ok(signature.result), place: Place::Annotation }
    }
}

/// A function literal that is a member of the witness an impl declares: the name it is declared
/// by, the interface and target the impl registers, and, where the witness is made by a function
/// of the target's parameters, that function.
struct Witnessed {
    name: Symbol,
    interface: ExprId,
    target: ExprId,
    maker: Option<ExprId>,
}

impl<'a, 'db> Env<'a, 'db> {
    /// What the place of the function literal `literal` says of its signature, if it has one and
    /// the literal leaves something out: a function type it is annotated with, or the requirement
    /// it implements. Where the place cannot be read here, such as a requirement that names a `$`
    /// parameter of the impl no instantiation binds yet, why. A literal with another number of
    /// parameters than the annotation takes the types of those they have in common, and is a
    /// mismatch with the annotation for the rest.
    #[must_use]
    pub fn context(&self, literal: ExprId) -> Option<Result<Context, Unevaluated>> {
        if !self.leaves_out(literal) {
            return None;
        }
        let (decl, _) = self.declaration_of(literal);
        let annotation = decl.and_then(|decl| self.parsed.hir.decls[decl].ty);
        if let Some(annotation) = annotation {
            let signature = self.ty(annotation).map(|ty| ty.signature().map(Context::from));
            return signature.transpose();
        }
        let asked = self.asked(literal)?;
        Some(asked.and_then(|asked| self.context_from(literal, asked)))
    }

    /// Whether the function literal `literal` leaves out what its place could give: a parameter
    /// written without a type, or its result.
    fn leaves_out(&self, literal: ExprId) -> bool {
        let ExprKind::Fn { returns, .. } = &self.parsed.hir.exprs[literal].kind else { return false };
        returns.is_none() || self.omits_param_types(literal)
    }

    /// Whether the function literal `literal` writes a run-time parameter without a type.
    #[must_use]
    pub fn omits_param_types(&self, literal: ExprId) -> bool {
        let hir = &self.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[literal].kind else { return false };
        params.iter().any(
            |p| matches!(p, Entry::One(Param { pat: Some(pat), ty: None, .. }) if !hir.is_comptime(*pat)),
        )
    }

    /// Where the interface writes the type of the requirement the function literal `literal`
    /// implements, if it is a member of the witness of an impl: why it is not known here, when
    /// the requirement names what the impl binds and no instantiation does.
    #[must_use]
    pub fn asked(&self, literal: ExprId) -> Option<Result<Written<'a, 'db>, Unevaluated>> {
        let witnessed = self.witnessed(literal)?;
        Some(self.requirement(&witnessed))
    }

    /// The witness `literal` is a member of, if it is: the value of the declaration an impl
    /// registers with `lang.impl(I(T))`, or the body of the function that makes it.
    fn witnessed(&self, literal: ExprId) -> Option<Witnessed> {
        let hir = &self.parsed.hir;
        let decl = self.declaration_of(literal).0?;
        let PatKind::Bind(binder) = hir.pats[hir.decls[decl].pat].kind else { return None };
        let witness = hir.struct_of(hir.member_ns(binder)?)?;
        let (declaration, maker) = if let Some(declaration) = self.declaration_of(witness).0 {
            (declaration, None)
        } else {
            let maker = hir.enclosing_function(witness)?;
            (self.declaration_of(maker).0?, Some(maker))
        };
        let (interface, target) =
            hir.decls[declaration].attrs.iter().find_map(|&attr| self.registered(attr))?;
        Some(Witnessed { name: hir.binders[binder].name, interface, target, maker })
    }

    /// The interface and target expressions `attr` registers a witness for, if it is
    /// `lang.impl(I(T))` for an interface `I`.
    fn registered(&self, attr: ExprId) -> Option<(ExprId, ExprId)> {
        let hir = &self.parsed.hir;
        let ExprKind::Call { callee, args } = &hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::r#impl))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        let ExprKind::Call { callee: interface, args } = &hir.exprs[*value].kind else { return None };
        let [Entry::One(Arg { name: None, value: target })] = args.as_slice() else { return None };
        matches!(self.eval(*interface), Ok(Value::Interface(_))).then_some((*interface, *target))
    }

    /// The function type the interface of `witnessed` writes for the requirement of the member's
    /// name, in the environment it is evaluated in.
    fn requirement(&self, witnessed: &Witnessed) -> Result<Written<'a, 'db>, Unevaluated> {
        let hir = &self.parsed.hir;
        let interface = self.eval(witnessed.interface)?;
        // The type the impl is for: the target, or what it makes of the parameters the witness's
        // function binds.
        let target = match witnessed.maker {
            None => self.eval(witnessed.target)?,
            Some(maker) => {
                let ExprKind::Fn { params, .. } = &hir.exprs[maker].kind else {
                    return Err(Unevaluated::Absent);
                };
                let args = params
                    .iter()
                    .map(|p| match p {
                        Entry::One(Param { pat: Some(pat), .. }) => match hir.pats[*pat].kind {
                            PatKind::Bind(b) => self.bound(b).cloned().ok_or(Unevaluated::Unbound),
                            _ => Err(Unevaluated::Unsupported),
                        },
                        _ => Err(Unevaluated::Unsupported),
                    })
                    .collect::<Result<_, _>>()?;
                self.apply(self.eval(witnessed.target)?, args)?
            }
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

    /// What the requirement `asked` says of the signature of `literal`, a member implementing it:
    /// its `$` parameters bound to those `literal` binds at their positions. Each part whose
    /// type is not known here is why.
    fn context_from(&self, literal: ExprId, asked: Written<'_, '_>) -> Result<Context, Unevaluated> {
        let hir = &self.parsed.hir;
        let parsed = Arc::clone(&asked.env.parsed);
        let their_hir = &parsed.hir;
        let ExprKind::FnType { params: required, returns, .. } = &their_hir.exprs[asked.expr].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let ExprKind::Fn { params: written, .. } = &hir.exprs[literal].kind else {
            return Err(Unevaluated::Absent);
        };
        let mut bound = Vec::new();
        for (required, written) in required.iter().zip(written) {
            let (Entry::One(Param { pat: Some(required), .. }), Entry::One(Param { pat: Some(written), .. })) =
                (required, written)
            else {
                continue;
            };
            let (&PatKind::Bind(required), &PatKind::Bind(written)) =
                (&their_hir.pats[*required].kind, &hir.pats[*written].kind)
            else {
                continue;
            };
            // A `$` parameter no instantiation binds yet leaves the types that name it unknown.
            if their_hir.binders[required].comptime
                && let Some(value) = self.bound(written).cloned()
            {
                let origin = their_hir.binders[required].origin;
                bound.push(Staging { binder: Node { id: required, origin }, value });
            }
        }
        let env = asked.env.staging(&bound);
        let params = required.iter().map(|p| env.declared_param(p)).collect();
        let result = returns.map_or_else(|| Ok(TyKind::unit()), |r| env.ty(r));
        Ok(Context { params, result, place: Place::Requirement })
    }
}
