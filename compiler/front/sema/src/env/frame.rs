//! Coroutine functions and their frames (spec 06): a function whose body yields returns a frame,
//! of a type implementing the interface it declares; what the frame yields and returns come from
//! that interface's `resume` (the table `iteration`). Also the signatures of function literals
//! and how long a call holds each parameter's borrow.

use vun_grammar::{BORROWS, Extent, ITERATION, TokenKind};
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Param, PatKind};
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::Cx;

use super::{Env, Unevaluated};
use crate::eval::{Call, Func, Instance};
use crate::item::ExprRef;
use crate::member::{Found, MemberType, Members};
use crate::ty::{Frame, Marker, Param as ParamTy, Signature, Ty, TyKind, Value};

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
                    written: returns.and_then(|r| self.written_call(r)),
                    name,
                };
                Value::Type(TyKind::Frame(frame).intern())
            }
            Some(value) => value,
            None => Value::Type(TyKind::unit()),
        };
        let mut signature = self.signature(params, None)?;
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
        let &(_, _, _, resume, ..) = ITERATION.first()?;
        Some(self.frame_member(frame, Symbol::intern(resume))?.signature()?.result)
    }

    /// The type of the member `name` a frame of type `frame` answers itself: a requirement of
    /// the interface it implements, such as `resume`.
    #[must_use]
    pub fn frame_member(&self, frame: Ty, name: Symbol) -> Option<Ty> {
        let TyKind::Frame(Frame { interface, .. }) = &*frame else { return None };
        let requirements =
            self.cx.get::<Call>(&Instance { f: interface.clone(), args: vec![Value::Type(frame)] });
        let requirements = requirements.ok()?.as_type()?;
        Some(self.cx.get::<MemberType>(&(requirements, name)).ok()?.ty)
    }

    /// What frames of type `frame` yield and return: the payloads of the variants `item` and
    /// `done` of what resuming them gives.
    #[must_use]
    pub fn frame_step(&self, frame: Ty) -> Option<(Ty, Ty)> {
        let &(.., item, done) = ITERATION.first()?;
        let members = self.cx.get::<Members>(&self.resumed(frame)?)?;
        let payload = |variant: &str| match members.find(Symbol::intern(variant)) {
            Found::Variant(v) => v.payload.first()?.as_ref().ok().copied(),
            _ => None,
        };
        Some((payload(item)?, payload(done)?))
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

impl Func {
    /// How long a call holds each parameter's borrow, as its annotation writes the mode
    /// (`&temp T`); `tied` for a parameter that is not a borrow. `None` when its file is not read.
    #[must_use]
    pub fn extents(&self, cx: &Cx<'_>) -> Option<Vec<Extent>> {
        let env = Env::inside(cx, self, [])?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[self.literal.expr.id].kind else { return None };
        let extent = |param: &Entry<Param>| {
            let Entry::One(Param { ty: Some(ty), .. }) = param else { return Extent::Tied };
            let ExprKind::Prefix { op: TokenKind::Amp, mode, .. } = hir.exprs[*ty].kind else {
                return Extent::Tied;
            };
            let word = mode.map_or("", Symbol::as_str);
            BORROWS.iter().find(|(m, ..)| *m == word).map_or(Extent::Tied, |&(.., extent)| extent)
        };
        Some(params.iter().map(extent).collect())
    }
}
