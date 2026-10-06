//! Folding the calls known when compiling in a body (spec 13, section 3).

use vun_primitives::Primitive;
use vun_primitives::lower::Lower;
use vun_sema::attributes::Attributed;
use vun_sema::callable::Callable;
use vun_sema::env::Env;
use vun_sema::target::TargetOf;
use vun_sema_ty::Value;

use crate::{Aggregate, Body, Operand, Rvalue};

impl Body {
    /// Rewrites the calls known when compiling: a variant's constructor builds the variant, a
    /// primitive whose parameters are all `$` ones, given constants, is its value, which nothing
    /// at run time changes, as is the integer a function of `$` parameters alone gives, and a
    /// primitive marked `lower.by(f)` is a call of `f`. Gives whether any changed.
    pub(super) fn fold(&mut self, env: &Env<'_, '_>) -> bool {
        let mut changed = false;
        for (_, block) in self.blocks.iter_mut() {
            for stmt in &mut block.stmts {
                let Rvalue::Call { callee: Operand::Const(callee), args } = &stmt.value else { continue };
                let folded = match callee {
                    Value::Constructor { ty, name } => Some(Rvalue::Aggregate {
                        kind: Aggregate::Variant { ty: *ty, name: *name },
                        parts: args.clone(),
                    }),
                    Value::Primitive(p) => {
                        Self::known(env, callee, args).map(|v| Rvalue::Use(Operand::Const(v))).or_else(|| {
                            let f = Self::lowered_by(env, *p, args)?;
                            Some(Rvalue::Call { callee: Operand::Const(f), args: args.clone() })
                        })
                    }
                    // A function of `$` parameters alone, as typing took it: an integer it gives is
                    // its value, whatever runs.
                    Value::Fn(_) => Self::known(env, callee, args)
                        .filter(|v| matches!(v, Value::Int(_)))
                        .map(|v| Rvalue::Use(Operand::Const(v))),
                    _ => None,
                };
                if let Some(folded) = folded {
                    stmt.value = folded;
                    changed = true;
                }
            }
        }
        changed
    }

    /// The function the primitive `p` applied to `args` runs as where no instruction computes
    /// it: always (`lower.by`), or where the target's processor does not compute with a type
    /// one of its constant arguments names (`lower.soft`): a float it lacks, an integer wider
    /// than an address. A function, so that folding it again changes nothing.
    fn lowered_by(env: &Env<'_, '_>, p: Primitive, args: &[Operand]) -> Option<Value> {
        let attributes = env.primitive_decl(p).ok()?.attributes(env.cx)?;
        let function = |attribute| {
            attributes
                .get(Primitive::Lower(attribute))
                .and_then(|mut f| f.pop())
                .filter(|f| matches!(f, Value::Fn(_)))
        };
        if let Some(f) = function(Lower::by) {
            return Some(f);
        }
        let target = env.cx.get::<TargetOf>(&())?;
        let lacked = args.iter().any(|a| matches!(a, Operand::Const(value) if !target.computes(value)));
        if lacked { function(Lower::soft) } else { None }
    }

    /// The value the primitive or function `callee` gives for `args`, when every parameter of it is
    /// a `$` one and every argument a constant: a call typing knew when compiling
    /// ([`Callable::folds`]).
    fn known(env: &Env<'_, '_>, callee: &Value, args: &[Operand]) -> Option<Value> {
        if !Callable::of(env, callee.clone())?.folds(env) {
            return None;
        }
        let args = args
            .iter()
            .map(|a| match a {
                Operand::Const(v) => Some(v.clone()),
                Operand::Place(_) => None,
            })
            .collect::<Option<_>>()?;
        env.apply(callee.clone(), args).ok()
    }
}
