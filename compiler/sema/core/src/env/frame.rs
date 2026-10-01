//! Coroutine functions and their frames (spec 06): a function whose body yields returns a frame,
//! of a type implementing the interface it declares; what the frame yields and returns come from
//! that interface's `resume` (the table `iteration`). Also the signatures of function literals
//! and how long a call holds each parameter's borrow.

use vun_desugar::{Borrow, CALLS, Extent, ITERATION};
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Param, PatKind};
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::Cx;
use vun_sema_ty::call::{Func, Instance};
use vun_sema_ty::item::ExprRef;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Frame, Marker, Param as ParamTy, Signature, Ty, TyKind, Value};

use super::{Context, Env};
use crate::callable::Parameters;
use crate::eval::Call;
use crate::member::{Found, MemberType, Members};

impl Env<'_, '_> {
    /// This environment with each `$` parameter of the function literal `literal` that nothing
    /// binds and an interface constrains bound to that parameter as the generic body sees it
    /// (`TyKind::Param`), so that the body is typed against the interface (spec 02, section 3).
    #[must_use]
    pub fn abstracted(mut self, literal: ExprId) -> Self {
        let parsed = std::sync::Arc::clone(&self.parsed);
        let hir = &parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[literal].kind else { return self };
        for param in params {
            let Entry::One(Param { pat: Some(pat), ty: Some(annotation), .. }) = param else { continue };
            let PatKind::Bind(binder) = hir.pats[*pat].kind else { continue };
            if !hir.binders[binder].comptime || self.bindings.contains_key(&binder) {
                continue;
            }
            if let Ok(Value::Interface(interface)) = self.eval(*annotation) {
                let param = ParamTy {
                    literal: ExprRef::new(self.file.clone(), hir, literal),
                    name: hir.binders[binder].name,
                    interface,
                };
                self.bindings.insert(binder, Value::Type(TyKind::Param(param).intern()));
            }
        }
        self
    }

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
        let context = self.context(literal).transpose()?;
        self.literal_signature_in(literal, args, context.as_ref())
    }

    /// The signature of the function literal `literal` as `literal_signature` gives it, within
    /// `context`, what its place says of what it leaves out.
    ///
    /// # Errors
    /// As [`Self::signature`].
    pub fn literal_signature_in(
        &self,
        literal: ExprId,
        args: &[Option<Value>],
        context: Option<&Context>,
    ) -> Result<Signature, Unevaluated> {
        let hir = &self.parsed.hir;
        let ExprKind::Fn { params, returns, .. } = &hir.exprs[literal].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let declared = returns.map(|r| self.eval(r)).transpose()?;
        let result = match declared {
            Some(Value::Interface(interface)) if hir.suspends(literal) => {
                let name = OriginTable::global().declaration_name(hir.exprs[literal].origin);
                let frame = Frame {
                    literal: ExprRef::new(self.file.clone(), hir, literal),
                    within: self.instance.clone(),
                    staged: self.staged().into(),
                    args: args.to_vec(),
                    interface,
                    written: returns.and_then(|r| self.written_call(r)),
                    name,
                };
                Value::Type(TyKind::Frame(Box::new(frame)).intern())
            }
            Some(value) => value,
            None => match context {
                Some(Context { result: Some(result), .. }) => Value::Type(result.clone()?),
                _ => Value::Type(TyKind::unit()),
            },
        };
        let mut signature = self.signature_in(params, None, context)?;
        signature.result = result.as_type().ok_or(Unevaluated::Unsupported)?;
        Ok(self.literal_markers(literal).into_iter().fold(signature, Signature::with))
    }

    /// The markers of the function literal `literal`: its attributes and its declaration's that
    /// are markers, and those the prefixes around it put, as `unsafe (p: *T) -> T { ... }`.
    #[must_use]
    pub fn literal_markers(&self, literal: ExprId) -> Vec<Marker> {
        let hir = &self.parsed.hir;
        let mut markers = self.markers(&self.attrs_of(literal));
        let prefixes = hir.exprs.iter().filter_map(|(_, e)| match &e.kind {
            ExprKind::Prefix { op, operand, .. } if *operand == literal => Marker::of_prefix(*op),
            _ => None,
        });
        for marker in prefixes {
            Marker::insert(&mut markers, marker);
        }
        markers
    }

    /// What resuming a frame of type `frame` gives: the result of its interface's `resume`, such
    /// as `lang.Step(u32, ())`.
    #[must_use]
    pub fn resumed(&self, frame: Ty) -> Option<Ty> {
        Some(self.frame_member(frame, ITERATION.first()?.resume.symbol())?.signature()?.result)
    }

    /// The type of the member `name` a frame of type `frame` answers itself: a requirement of
    /// the interface it implements, such as `resume`.
    #[must_use]
    pub fn frame_member(&self, frame: Ty, name: Symbol) -> Option<Ty> {
        let requirements = self.required(frame).filter(|_| matches!(*frame, TyKind::Frame(_)))?;
        Some(self.cx.get::<MemberType>(&(requirements, name)).ok()?.ty)
    }

    /// The requirements of the interface `ty` is known by, made for `ty`: what a parameter of a
    /// generic body, a frame, or a function or closure has as members. A function or closure is
    /// known by the interface calls go through (the table `calls`), made for its parameters and
    /// result.
    #[must_use]
    pub fn required(&self, ty: Ty) -> Option<Ty> {
        let interface = ty.interface().cloned().or_else(|| self.called_through(ty))?;
        let call = Instance { f: interface, args: vec![Value::Type(ty)] };
        self.cx.get::<Call>(&call).ok()?.as_type()
    }

    /// The interface calls go through (the table `calls`), made for the parameters and result of
    /// the function or closure type `ty`.
    fn called_through(&self, ty: Ty) -> Option<Func> {
        let constructor = self.item(CALLS.first()?.parent()?).ok()?;
        let args = self.implemented(ty, &constructor)?;
        let Value::Fn(f) = constructor else { return None };
        match self.cx.get::<Call>(&Instance { f, args }).ok()? {
            Value::Interface(interface) => Some(interface),
            _ => None,
        }
    }

    /// What frames of type `frame` yield and return: the payloads of the variants `item` and
    /// `done` of what resuming them gives.
    #[must_use]
    pub fn frame_step(&self, frame: Ty) -> Option<(Ty, Ty)> {
        let row = ITERATION.first()?;
        let members = self.cx.get::<Members>(&self.resumed(frame)?)?;
        let payload = |variant: vun_items::Item| match members.find(variant.symbol()) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied(),
            _ => None,
        };
        Some((payload(row.each)?, payload(row.done)?))
    }
}

impl Env<'_, '_> {
    /// The call `expr` writes, its callee and arguments evaluated, when it is a call of a
    /// function value with plain arguments.
    fn written_call(&self, expr: ExprId) -> Option<Instance> {
        let ExprKind::Call { callee, args } = &self.parsed.hir.exprs[expr].kind else { return None };
        let Ok(Value::Fn(f)) = self.eval(*callee) else { return None };
        // A tuple of types, such as `()`, is the tuple type, as a type argument means it.
        let args = args.iter().map(|a| match a {
            Entry::One(Arg { name: None, value }) => match self.eval(*value).ok()? {
                tuple @ Value::Tuple(_) => Some(tuple.as_type().map_or(tuple, Value::Type)),
                value => Some(value),
            },
            _ => None,
        });
        Some(Instance { f, args: args.collect::<Option<_>>()? })
    }
}

/// The extents of a function's parameters.
pub trait Extents {
    /// How long a call holds each parameter's borrow, as its annotation writes the mode
    /// (`&temp T`); `tied` for a parameter that is not a borrow. `None` when its file is not read.
    #[must_use]
    fn extents(&self, cx: &Cx<'_>) -> Option<Vec<Extent>>;
}

impl Extents for Func {
    fn extents(&self, cx: &Cx<'_>) -> Option<Vec<Extent>> {
        Some(self.modes(cx)?.into_iter().map(|mode| mode.map_or(Extent::Tied, Borrow::extent)).collect())
    }
}
