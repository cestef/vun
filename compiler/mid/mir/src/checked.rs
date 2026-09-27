//! Checked operations (spec 13, section 8.7): under the program's checks, a call of what
//! `#[lang.check.by(kind, f)]` marks is a call of `f`, with the call's origin after its
//! arguments; within `f` itself, the operation it checks runs unchecked.

use vun_sanitize::Checks;
use vun_sema::env::Env;
use vun_sema::eval::Func;
use vun_sema::ty::Value;

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
                let attributes = match callee {
                    Value::Primitive(p) => env.primitive_decl(*p).ok().and_then(|d| d.attributes(env.cx)),
                    Value::Fn(f) => f.attributes(env.cx),
                    _ => None,
                };
                let Some(version) = attributes.and_then(|a| checks.version(&a)) else { continue };
                if matches!(&version, Value::Fn(f) if f == within) {
                    continue;
                }
                *callee = version;
                args.push(Operand::Const(Value::Origin(stmt.origin)));
            }
        }
    }
}
