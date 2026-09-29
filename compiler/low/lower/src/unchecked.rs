//! The functions the checks of unsafe code run in (spec 13, section 8.7): each function a
//! runtime exports for an operation of `lang.check`, and every function one calls. The checks do
//! not apply in them, or an operation would check itself.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hash::{Set, Stable};
use vun_hir::db::FileRef;
use vun_mir::{Concrete, Lowered as _, Operand, Program, Rvalue};
use vun_query::Cx;
use vun_sanitize::Operations;
use vun_sema::checks::ChecksOf;
use vun_sema_ty::Value;
use vun_sema_ty::call::Instantiation;

use crate::lowering::Lowering;

/// The functions of a program the checks run in, in an order of their own: alike for every
/// program whose runtime is the same, which a function lowered on its own is keyed by.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unchecked(Arc<Vec<Instantiation>>);

impl Unchecked {
    /// Whether the checks do not apply in `f`.
    pub(crate) fn contains(&self, f: &Instantiation) -> bool {
        self.0.contains(f)
    }
}

impl Lowering<'_, '_> {
    /// The functions of `program`, rooted at `root`, the checks run in; none without checks.
    pub(crate) fn unchecked(cx: &Cx<'_>, root: &FileRef, program: &Program) -> Unchecked {
        let mut unchecked = Set::default();
        if cx.get::<ChecksOf>(&()).is_empty() {
            return Unchecked::default();
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
        let mut unchecked: Vec<Instantiation> = unchecked.into_iter().collect();
        unchecked.sort_by_key(Stable::of);
        Unchecked(Arc::new(unchecked))
    }
}
