//! C records by value: loaded into the registers they travel in and stored back from them, copied
//! where the convention has the caller copy them, and the address a result in memory comes back
//! through (`plan`).

use cranelift_codegen::ir::{InstBuilder, StackSlotData, StackSlotKind, Value as Val};
use vun_lir::Value;

use super::Lower;
use crate::abi::Plan;
use crate::plan::{Layout, Parts, Pass, Returned};

impl Lower<'_, '_> {
    /// A fresh stack place for a record of `layout`, and its address.
    fn scratch(&mut self, layout: Layout) -> Val {
        let shift = u8::try_from(layout.align.max(1).trailing_zeros()).expect("alignments are powers of 2");
        let size = u32::try_from(layout.size).expect("records fit u32");
        let slot =
            self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, shift));
        self.b.ins().stack_addr(self.ptr, slot, 0)
    }

    /// The values a call passes as `plan` says, for LIR's `args`, and where a record returned in
    /// registers is stored.
    pub(super) fn arguments(&mut self, plan: &Plan, args: &[Value]) -> (Vec<Val>, Option<Val>) {
        let mut values = self.get_all(args).into_iter();
        let mut passed = Vec::new();
        let result = match plan.returned {
            Returned::Sret | Returned::Parts(_) => {
                let at = values.next_back().expect("a result in memory has an address");
                if plan.returned == Returned::Sret {
                    passed.push(at);
                }
                Some(at)
            }
            Returned::Direct | Returned::Through => None,
        };
        for pass in &plan.params {
            match pass {
                Pass::Values(count) => passed.extend(values.by_ref().take(*count)),
                Pass::Parts(parts) => {
                    let at = values.next().expect("a record has an address");
                    passed.extend(self.load_parts(parts, at));
                }
                Pass::Struct(_) => passed.extend(values.next()),
                Pass::Copy(layout) => {
                    let from = values.next().expect("a record has an address");
                    let copy = self.scratch(*layout);
                    self.copy(copy, from, layout.size);
                    passed.push(copy);
                }
            }
        }
        passed.extend(values);
        (passed, result)
    }

    /// The parts of the record at `at`, each as the scalar its register holds.
    pub(super) fn load_parts(&mut self, parts: &Parts, at: Val) -> Vec<Val> {
        let flags = Self::plain();
        let load = |this: &mut Self, lane: &crate::plan::Lane| {
            let offset = i32::try_from(lane.offset).expect("records are within a signed offset");
            this.b.ins().load(lane.ty, flags, at, offset)
        };
        parts.lanes.iter().map(|lane| load(self, lane)).collect()
    }

    /// Stores `values`, the parts of a record, at `at`.
    pub(super) fn store_parts(&mut self, parts: &Parts, at: Val, values: &[Val]) {
        let flags = Self::plain();
        for (lane, &value) in parts.lanes.iter().zip(values) {
            let offset = i32::try_from(lane.offset).expect("records are within a signed offset");
            self.b.ins().store(flags, value, at, offset);
        }
    }

    /// Defines the entry block's LIR `params` from what the function receives: a record in
    /// registers is stored in a place of its own, and a result returned in registers is written to
    /// one, which the return reads.
    pub(super) fn bind(&mut self, params: &[Value], passed: Vec<Val>) {
        let plan = self.plan;
        let mut params = params.iter();
        let mut passed = passed.into_iter();
        let sret = (plan.returned == Returned::Sret).then(|| passed.next().expect("a result address"));
        for pass in &plan.params {
            match pass {
                Pass::Values(count) => {
                    for (&param, value) in params.by_ref().zip(passed.by_ref()).take(*count) {
                        self.values[param] = Some(value);
                    }
                }
                Pass::Parts(parts) => {
                    let at = self.scratch(parts.layout);
                    let received: Vec<_> = passed.by_ref().take(parts.lanes.len()).collect();
                    self.store_parts(parts, at, &received);
                    self.values[*params.next().expect("a parameter per record")] = Some(at);
                }
                Pass::Struct(_) | Pass::Copy(_) => {
                    self.values[*params.next().expect("a parameter per record")] = passed.next();
                }
            }
        }
        match &plan.returned {
            Returned::Sret => self.values[*params.next().expect("a result address")] = sret,
            Returned::Parts(parts) => {
                let at = self.scratch(parts.layout);
                self.returning = Some((at, parts));
                self.values[*params.next().expect("a result address")] = Some(at);
            }
            Returned::Direct | Returned::Through => {
                for (&param, value) in params.zip(passed) {
                    self.values[param] = Some(value);
                }
            }
        }
    }
}
