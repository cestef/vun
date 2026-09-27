//! Checked operations (spec 13, section 8.7): under the program's checks, a call of what
//! `#[lang.check.by(kind, f)]` marks is a call of `f`, with the call's origin after its
//! arguments; a call of a function implementing a requirement so marked is one too, with the
//! type the impl is for before them. Within `f` itself, the operation it checks runs unchecked.

use vun_sema::env::Env;
use vun_sema::eval::Func;
use vun_sema::ty::Value;
use vun_target::Checks;

use crate::{Body, Operand, Rvalue};

impl Body {
    /// Rewrites each call of an operation `checks` check into a call of its checked version,
    /// but in `within`'s own body when it is that version.
    pub(crate) fn checked(&mut self, env: &Env<'_, '_>, checks: &Checks, within: &Func) {
        if checks.is_empty() {
            return;
        }
        for (_, block) in self.blocks.iter_mut() {
            for stmt in &mut block.stmts {
                let Rvalue::Call { callee: Operand::Const(callee), args } = &mut stmt.value else { continue };
                let Some((version, target)) = Self::version(env, callee, checks) else { continue };
                if matches!(&version, Value::Fn(f) if f == within) {
                    continue;
                }
                *callee = version;
                if let Some(target) = target {
                    args.insert(0, Operand::Const(target));
                }
                args.push(Operand::Const(Value::Origin(stmt.origin)));
            }
        }
    }

    /// What a call of `callee` runs as under `checks`: the version its own attributes name, or
    /// for a function implementing a requirement, the one the requirement's name, which takes the
    /// type the impl is for first.
    fn version(env: &Env<'_, '_>, callee: &Value, checks: &Checks) -> Option<(Value, Option<Value>)> {
        let cx = env.cx;
        match callee {
            Value::Primitive(p) => {
                Some((env.primitive_decl(*p).ok()?.attributes(cx)?.checked(checks)?, None))
            }
            Value::Fn(f) => {
                f.attributes(cx).and_then(|a| a.checked(checks)).map(|v| (v, None)).or_else(|| {
                    let implemented = f.requirement(cx)?;
                    let version = implemented.requirement.attributes(cx)?.checked(checks)?;
                    Some((version, Some(implemented.target)))
                })
            }
            _ => None,
        }
    }
}
