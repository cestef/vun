//! The types annotations denote, and the signatures they write: what a function declares of its
//! parameters and its result, and what its place gives the parts it leaves out.

use vun_hir::tree::{Entry, ExprId, List, Param, PatId, PatKind};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Ty, TyKind, Value};

use super::{Context, Env};

impl Env<'_, '_> {
    /// The type `expr` denotes, evaluated where a type is expected.
    ///
    /// # Errors
    /// Returns why it has no value here, or [`Unevaluated::Unsupported`] for a value that is not
    /// a type.
    pub fn ty(&self, expr: ExprId) -> Result<Ty, Unevaluated> {
        self.eval(expr)?.as_type().ok_or(Unevaluated::Unsupported)
    }

    /// The result type a function declares: `()` when it declares none.
    ///
    /// # Errors
    /// As [`Self::ty`].
    pub fn result(&self, returns: Option<ExprId>) -> Result<Ty, Unevaluated> {
        returns.map_or_else(|| Ok(TyKind::unit()), |r| self.ty(r))
    }

    /// The signature `params -> returns` declares.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value here, such as a type naming a `$`
    /// parameter, known only per instantiation.
    pub fn signature(&self, params: &List<Param>, returns: Option<ExprId>) -> Result<Signature, Unevaluated> {
        self.signature_in(params, returns, None)
    }

    /// The signature `params -> returns` declares within `context`, what its place says of it: a
    /// parameter written without a type has the type `context` gives its position, and a result
    /// not written is `context`'s.
    ///
    /// # Errors
    /// As [`Self::signature`].
    pub fn signature_in(
        &self,
        params: &List<Param>,
        returns: Option<ExprId>,
        context: Option<&Context>,
    ) -> Result<Signature, Unevaluated> {
        let params = self.params_in(params, context)?;
        let result = match (returns, context) {
            (None, Some(Context { result: Some(result), .. })) => result.clone()?,
            _ => self.result(returns)?,
        };
        Ok(Signature::new(params, result))
    }

    /// The types of the written `params` within `context`: a parameter written without a type has
    /// the type `context` gives its position.
    ///
    /// # Errors
    /// Returns why a parameter has no type here.
    pub fn params_in(&self, params: &List<Param>, context: Option<&Context>) -> Result<Vec<Ty>, Unevaluated> {
        params
            .iter()
            .enumerate()
            .map(|(index, param)| match param {
                Entry::One(Param { pat: Some(pat), ty: None, .. }) if !self.parsed.hir.is_comptime(*pat) => {
                    context.and_then(|c| c.param(index)).unwrap_or(Err(Unevaluated::Unsupported))
                }
                _ => self.declared_param(param),
            })
            .collect()
    }

    /// The type the written parameter `param` has: a pack's for `..args: P`, and for `$W:
    /// Writer`, constrained by an interface, a type.
    ///
    /// # Errors
    /// Returns why the annotation has no value here, or that the parameter has none.
    pub fn declared_param(&self, param: &Entry<Param>) -> Result<Ty, Unevaluated> {
        match param {
            Entry::One(Param { pat, ty: Some(t), .. }) => match self.eval(*t) {
                Ok(Value::Interface(_)) => Ok(TyKind::of_types()),
                _ => self.param_type(*pat, self.ty(*t)?),
            },
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The type a function's signature gives the parameter `pat` annotated `ty`: a pack's,
    /// `lang.Pack(ty)`, for the rest pattern `..args` gathering the arguments after the others.
    ///
    /// # Errors
    /// Returns why `lang.Pack(ty)` has no value: a type that is not a tuple.
    pub fn param_type(&self, pat: Option<PatId>, ty: Ty) -> Result<Ty, Unevaluated> {
        match pat.map(|p| &self.parsed.hir.pats[p].kind) {
            Some(PatKind::Rest(_)) => {
                let pack = self.item(vun_items::lang::Pack)?;
                self.apply(pack, vec![Value::Type(ty)])?.as_type().ok_or(Unevaluated::Unsupported)
            }
            _ => Ok(ty),
        }
    }
}
