//! Coroutine functions and their frames (spec 06): a function whose body yields returns a frame,
//! of a type implementing the interface it declares; what the frame yields and returns come from
//! that interface's `resume` (the table `iteration`).

use overt_grammar::ITERATION;
use overt_hir::tree::{ExprId, ExprKind};
use overt_intern::Symbol;
use overt_origin::OriginTable;

use super::{Env, Unevaluated};
use crate::eval::{Call, Instance};
use crate::item::ExprRef;
use crate::member::{Found, MemberType, Members};
use crate::ty::{Frame, Signature, Ty, TyKind, Value};

impl Env<'_, '_> {
    /// The signature of the function literal `literal` here, `args` the values of its `$`
    /// parameters where an instantiation binds them: a coroutine function returns its frame.
    ///
    /// # Errors
    /// As [`Self::signature`].
    pub fn literal_signature(
        &self,
        literal: ExprId,
        args: &[Option<Value>],
    ) -> Result<Signature, Unevaluated> {
        let hir = &self.parsed.hir;
        let ExprKind::Fn { params, returns, .. } = &hir.exprs[literal].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let declared = returns.map(|r| self.eval(r)).transpose()?;
        let result = match declared {
            Some(Value::Interface(interface)) if hir.suspends(literal) => {
                let name = self.cx.db().resource::<OriginTable>().declaration_name(hir.exprs[literal].origin);
                let frame = Frame {
                    literal: ExprRef::new(self.file.clone(), hir, literal),
                    within: self.instance.clone(),
                    args: args.to_vec(),
                    interface,
                    name,
                };
                Value::Type(TyKind::Frame(frame).intern())
            }
            Some(value) => value,
            None => Value::Type(TyKind::unit()),
        };
        let mut signature = self.signature(params, None)?;
        signature.result = result.as_type().ok_or(Unevaluated::Unsupported)?;
        // `unsafe (p: *T) -> T { ... }`: the prefixes around the literal mark its type.
        let marking = hir.exprs.iter().filter_map(|(_, e)| match &e.kind {
            ExprKind::Prefix { op, operand, .. } if *operand == literal => Some(*op),
            _ => None,
        });
        Ok(marking.fold(signature, Signature::marked))
    }

    /// What resuming a frame of type `frame` gives: the result of its interface's `resume`, such
    /// as `lang.Step(u32, ())`.
    #[must_use]
    pub fn resumed(&self, frame: Ty) -> Option<Ty> {
        let TyKind::Frame(Frame { interface, .. }) = &*frame else { return None };
        let &(_, _, _, resume, _, _) = ITERATION.first()?;
        let requirements =
            self.cx.get::<Call>(&Instance { f: interface.clone(), args: vec![Value::Type(frame)] });
        let requirements = requirements.ok()?.as_type()?;
        let resumed = self.cx.get::<MemberType>(&(requirements, Symbol::intern(resume))).ok()?;
        Some(resumed.ty.signature()?.result)
    }

    /// What frames of type `frame` yield and return: the payloads of the variants `item` and
    /// `done` of what resuming them gives.
    #[must_use]
    pub fn frame_step(&self, frame: Ty) -> Option<(Ty, Ty)> {
        let &(_, _, _, _, item, done) = ITERATION.first()?;
        let members = self.cx.get::<Members>(&self.resumed(frame)?)?;
        let payload = |variant: &str| match members.find(Symbol::intern(variant)) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied(),
            _ => None,
        };
        Some((payload(item)?, payload(done)?))
    }
}
