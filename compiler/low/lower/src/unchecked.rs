//! The functions the checks of unsafe code run in (spec 13, section 8.7): each function a
//! runtime exports for an operation of `lang.check`, and every function one calls. The checks do
//! not apply in them, or an operation would check itself.

use vun_hash::Set;
use vun_hir::db::FileRef;
use vun_mir::{Concrete, Lowered as _, Operand, Program, Rvalue};
use vun_query::Cx;
use vun_sanitize::Operations;
use vun_sema::checks::ChecksOf;
use vun_sema::eval::Instantiation;
use vun_sema::ty::Value;

use crate::lowering::Lowering;

impl Lowering<'_, '_> {
    /// The functions of `program`, rooted at `root`, the checks run in; none without checks.
    pub(crate) fn unchecked(cx: &Cx<'_>, root: &FileRef, program: &Program) -> Set<Instantiation> {
        let mut unchecked = Set::default();
        if cx.get::<ChecksOf>(&()).is_empty() {
            return unchecked;
        }
        let operations = cx.get::<Operations>(&root.package);
        let mut pending: Vec<Instantiation> = program
            .exports
            .iter()
            .filter(|(_, symbol)| operations.iter().any(|(_, s)| s == symbol))
            .map(|&(i, _)| program.functions[i].clone())
            .collect();
        while let Some(f) = pending.pop() {
            let Ok(body) = cx.get::<Concrete>(&f) else { continue };
            if !unchecked.insert(f) {
                continue;
            }
            let stmts = body.blocks.iter().flat_map(|(_, b)| &b.stmts);
            for stmt in stmts {
                if let Rvalue::Call { callee: Operand::Const(Value::Fn(callee)), args } = &stmt.value {
                    pending.push(Instantiation::called(cx, callee, args));
                }
            }
        }
        unchecked
    }
}
