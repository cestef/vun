//! Calls (spec 01, section 4): arguments are checked against the parameters of the callee's
//! signature. A signature whose types name `$` parameters, such as `($T: type, x: u8) -> T`, is
//! typed per call, inside the callee with those parameters bound to the call's arguments.

use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Param, PatKind};
use overt_hir::visit::Visitor;

use super::{Known, Typer};
use crate::env::Env;
use crate::member::MemberValue;
use crate::ty::{Signature, Value};

impl Typer<'_, '_, '_> {
    /// A call of a value of function type: its arguments are checked against the parameters, and
    /// it has the result type.
    pub(super) fn call(&mut self, callee: ExprId, args: &[Entry<Arg>]) -> Option<Known> {
        let known = self.typed(callee).and_then(|k| k.ty.signature());
        // A method's receiver is passed in the mode its first parameter asks for.
        let receiver = usize::from(self.methods.contains_key(&callee));
        let signature = known.or_else(|| self.dependent(callee, receiver, args));
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

    /// The signature of the function `callee` names, typed with its `$` parameters bound to the
    /// values of their arguments, which are known at compile time. The callee is a method, or a
    /// function known at compile time.
    fn dependent(&self, callee: ExprId, receiver: usize, args: &[Entry<Arg>]) -> Option<Signature> {
        let f = match self.methods.get(&callee) {
            Some(method) => self.env.cx.get::<MemberValue>(method),
            None => self.env.eval(callee),
        };
        let Ok(Value::Fn(f)) = f else { return None };
        let target = Env::of(self.env.cx, &f.literal.file)?;
        let hir = &target.parsed.hir;
        let ExprKind::Fn { params, returns, .. } = &hir.exprs[f.literal.expr.id].kind else { return None };
        let comptime = params.iter().map(|p| match p {
            Entry::One(Param { pat: Some(pat), .. }) => {
                matches!(hir.pats[*pat].kind, PatKind::Bind(b) if hir.binders[b].comptime)
            }
            _ => false,
        });
        let values = comptime.enumerate().map(|(i, comptime)| {
            let arg = i.checked_sub(receiver).and_then(|i| args.get(i));
            match (comptime, arg) {
                (true, Some(Entry::One(Arg { name: None, value }))) => self.env.eval(*value).ok(),
                _ => None,
            }
        });
        let inside = Env::inside(self.env.cx, &f, values.collect::<Vec<_>>())?;
        inside.signature(params, *returns).ok()
    }
}
