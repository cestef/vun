//! Calls (spec 01, section 4): arguments are checked against the parameters of the callee's
//! signature. A signature whose types name `$` parameters, such as `($T: type, x: u8) -> T`, is
//! typed per call, inside the callee with those parameters bound to the call's arguments.

use overt_diag::TraceFrame;
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind};
use overt_hir::visit::Visitor;
use overt_query::Cx;

use super::{Known, Typed, Typer};
use crate::env::Env;
use crate::eval::Instantiation;
use crate::member::MemberValue;
use crate::ty::{Signature, Value};

impl Typer<'_, '_, '_> {
    /// A call of a value of function type: its arguments are checked against the parameters, and
    /// it has the result type.
    pub(super) fn call(&mut self, call: ExprId, callee: ExprId, args: &[Entry<Arg>]) -> Option<Known> {
        let known = self.typed(callee).and_then(|k| k.ty.signature());
        // A method's receiver is passed in the mode its first parameter asks for.
        let receiver = usize::from(self.methods.contains_key(&callee));
        let instantiation = self.instantiation(callee, receiver, args);
        if let Some(i) = instantiation.as_ref().filter(|i| i.generic(self.env.cx))
            && let Some(typed) = self.env.cx.get::<Typed>(i)
        {
            let frame = TraceFrame {
                msg: "in the instantiation this call reaches".into(),
                origin: self.hir().exprs[call].origin,
            };
            self.diag.absorb_through(&typed.diag, &frame);
        }
        let signature = known.or_else(|| instantiation?.signature(self.env.cx));
        let hir = self.hir();
        let (mut found, mut positional) = (Vec::new(), true);
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) => {
                    let param =
                        signature.as_ref().and_then(|s| s.params.get(receiver + found.len()).copied());
                    found.push((*value, self.typed_as(*value, param)));
                }
                Entry::One(Arg { value, .. }) => {
                    positional = false;
                    self.typed(*value);
                }
                Entry::Staged(staged) => {
                    positional = false;
                    self.staged(hir, staged, &mut |v, a: &Arg| {
                        v.typed(a.value);
                    });
                }
            }
        }
        let signature = signature.filter(|_| positional)?;
        let params = signature.params.get(receiver..)?;
        if params.len() != found.len() {
            return None;
        }
        for (&param, (value, known)) in params.iter().zip(found) {
            if let Some(known) = known {
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        Some(Known { ty: signature.result, stage: self.stage })
    }

    /// The instantiation a call of `callee` with `args` reaches: the function it names, a
    /// method or a function known at compile time, with each `$` parameter bound to the value of
    /// its argument, when that is known here.
    fn instantiation(&self, callee: ExprId, receiver: usize, args: &[Entry<Arg>]) -> Option<Instantiation> {
        let f = match self.methods.get(&callee) {
            Some(method) => self.env.cx.get::<MemberValue>(method),
            None => self.env.eval(callee),
        };
        let Ok(Value::Fn(f)) = f else { return None };
        let comptime = f.comptime(self.env.cx)?;
        let args = comptime.iter().enumerate().map(|(i, &comptime)| {
            let arg = i.checked_sub(receiver).and_then(|i| args.get(i));
            match (comptime, arg) {
                (true, Some(Entry::One(Arg { name: None, value }))) => self.env.eval(*value).ok(),
                _ => None,
            }
        });
        Some(Instantiation { args: args.collect(), f })
    }
}

impl Instantiation {
    /// Whether this is an instantiation of generic code: a function built in a compile-time
    /// call, or one with `$` parameters, all bound.
    fn generic(&self, cx: &Cx<'_>) -> bool {
        let Some(comptime) = self.f.comptime(cx) else { return false };
        let bound = comptime.iter().zip(&self.args).all(|(&c, arg)| !c || arg.is_some());
        (self.f.within.is_some() || comptime.contains(&true)) && bound
    }

    /// The signature of this instantiation, its types evaluated with its `$` parameters bound.
    fn signature(&self, cx: &Cx<'_>) -> Option<Signature> {
        let inside = Env::inside(cx, &self.f, self.args.iter().cloned())?;
        let ExprKind::Fn { params, returns, .. } = &inside.parsed.hir.exprs[self.f.literal.expr.id].kind
        else {
            return None;
        };
        inside.signature(params, *returns).ok()
    }
}
