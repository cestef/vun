//! Lowering every function of a unit, in parallel, until the functions that tail call one
//! another are those no other backend compiles (`Tails`).

use vun_backend::Lack;
use vun_index::Idx;
use vun_lir::analysis::Reached;
use vun_lir::{FuncId, Function, Unit};
use vun_par::Parallel;

use super::Compilation;
use crate::func::{Cx, Lowered};
use crate::isa::Isa;
use crate::tail::Tails;

impl Compilation {
    /// Each function of `functions` lowered, or what Cranelift lacks of it; none for one not
    /// reached. A group of functions that tail call one another is lowered as bodies and entries
    /// only where every one lowers, which one lacking, known once all are lowered, takes away.
    pub(super) fn lowered(
        unit: &Unit,
        isa: &Isa,
        reached: &Reached,
        functions: &[(FuncId, &Function)],
    ) -> Vec<Option<Result<Lowered, Lack>>> {
        let mut tails = Tails::of(unit, reached, &[]);
        loop {
            let cx = Cx::new(unit, isa, reached, &tails);
            let lowered = functions.par_map(|&(id, f)| {
                if !reached.contains(id) {
                    return None;
                }
                let _span =
                    vun_trace::tracing::debug_span!("cranelift", id = id.index(), f = %f.linkage).entered();
                Some(cx.lower(id, f))
            });
            let lacking: Vec<FuncId> = lowered
                .iter()
                .zip(functions)
                .filter(|(l, _)| matches!(l, Some(Err(_))))
                .map(|(_, &(id, _))| id)
                .collect();
            let next = Tails::of(unit, reached, &lacking);
            if next == tails {
                return lowered;
            }
            tails = next;
        }
    }
}
