//! Wide intermediates (spec 04, section 1, "Cost model"): an arithmetic result whose range needs
//! more bits than the target's register holds, which a narrow target computes in several steps.
//! A measure for `vun check --widen`, not a fault.

use vun_codec::{Decode, Encode};
use vun_mir::{Mir, Operand, Rvalue};
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::int::Int;
use vun_query::{Cx, Query};
use vun_sema::target::TargetOf;
use vun_sema_ty::Value;
use vun_sema_ty::call::Instantiation;

/// The integer operations whose result is wider than their operands by rule 1 (spec 04).
const WIDENING: [Int; 4] = [Int::add, Int::sub, Int::mul, Int::neg];

/// An arithmetic result needing more bits than the register holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Wide {
    pub origin: OriginId,
    pub bits: u64,
    pub register: u32,
}

/// The wide intermediates of an instantiation's body; `None` when it has no MIR yet or the
/// compilation has no target, which a width is measured against.
pub struct WideChecks;

impl Query for WideChecks {
    const NAME: &'static str = "wide_checks";
    type Key = Instantiation;
    type Value = Option<Vec<Wide>>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let register = cx.get::<TargetOf>(&())?.register_bits;
        let body = cx.get::<Mir>(i).ok()?;
        let mut wide = Vec::new();
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                let Rvalue::Call { callee: Operand::Const(Value::Primitive(Primitive::Int(op))), .. } =
                    &stmt.value
                else {
                    continue;
                };
                if !WIDENING.contains(op) || !stmt.place.projection.is_empty() {
                    continue;
                }
                let bits =
                    body.locals[stmt.place.local].ty.and_then(|ty| ty.interval()).and_then(|r| r.bits());
                wide.extend(bits.filter(|&bits| bits > u64::from(register)).map(|bits| Wide {
                    origin: stmt.origin,
                    bits,
                    register,
                }));
            }
        }
        Some(wide)
    }
}
