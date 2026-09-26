//! The compilation of one function: the state the framework keeps as it walks the blocks, and
//! the walk.

mod call;
mod exit;
mod fixed;
mod values;

pub use exit::{Fused, Layout};

use vun_asm::{Isa, Label};
use vun_backend::{Lack, Missing};
use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Liveness, Loops, Order};
use vun_lir::{Address, Block, Conv, Function, Slot, Unit, Value};
use vun_origin::OriginId;

use crate::frame::Frame;
use crate::regs::RegFile;
use crate::values::{Assignment, assign};
use crate::{Arch, Code, Isel};

/// The state of the compilation of one function.
pub struct Cx<'a, A: Arch> {
    pub unit: &'a Unit,
    pub f: &'a Function,
    pub order: Order,
    pub cfg: Cfg,
    pub asm: Code<A>,
    pub frame: Frame,
    /// What the instruction compilers keep.
    pub state: A::State,
    /// Where the code emitted now comes from.
    pub at: OriginId,
    labels: IndexVec<Block, Label>,
    values: IndexVec<Value, Assignment<A::Reg>>,
    regs: RegFile<A>,
    slots: IndexVec<Slot, i32>,
    /// The block emitted next, when it starts with the registers as the last one left them.
    kept: Option<Block>,
    /// The callee-saved registers taken, with where the prologue saved each.
    saves: Vec<(A::Reg, i32)>,
    /// Where each instruction's code starts, with its origin.
    marks: Vec<(u64, OriginId)>,
}

/// A function's machine code.
pub struct Compiled<T> {
    pub code: vun_asm::Code<T, Address>,
    /// The bytes its frame takes below the frame pointer.
    pub frame: u64,
    /// Where the code of each instruction and terminator starts, with its origin: what line
    /// tables and listings read.
    pub marks: Vec<(u64, OriginId)>,
}

/// Compiles `f` of `unit`, listing its code when `listed`.
///
/// # Errors
/// Returns what the backend cannot compile.
///
/// # Panics
/// Never for a function whose frame and code fit the architecture's offsets.
pub fn compile<A: Isel>(
    unit: &Unit,
    f: &Function,
    listed: bool,
) -> Result<Compiled<<A::Isa as Isa>::Inst<Address>>, Lack> {
    if f.signature.conv == Conv::Naked {
        return Err(Missing::Naked.at(f.origin));
    }
    let cfg = Cfg::of(f);
    let loops = Loops::of(f, &cfg);
    let order = Order::of(f, &loops);
    let live = Liveness::of(f, &loops, &order);
    let mut asm = if listed { Code::<A>::listed() } else { Code::<A>::default() };
    let labels = f.blocks.iter().map(|_| asm.label()).collect::<Vec<_>>().into();
    let mut frame = Frame::default();
    let mut slots = IndexVec::new();
    for (_, slot) in f.slots.iter() {
        if slot.align > A::STACK_ALIGN {
            return Err(Missing::Align(slot.align).at(slot.origin));
        }
        slots.push(frame.reserve(slot.size, slot.align));
    }
    let values = assign(f, &live, &order, &slots);
    let mut cx = Cx {
        unit,
        f,
        state: A::state(f)?,
        order,
        cfg,
        asm,
        frame,
        at: f.origin,
        labels,
        values,
        regs: RegFile::new(),
        slots,
        kept: None,
        saves: Vec::new(),
        // The prologue comes from the function itself.
        marks: vec![(0, f.origin)],
    };
    A::prologue(&mut cx)?;
    cx.fix(&live, &loops);
    A::entry(&mut cx)?;
    for position in 0..cx.order.blocks().len() {
        let block = cx.order.blocks()[position];
        cx.block(block)?;
    }
    let size = cx.frame.size(A::STACK_ALIGN);
    A::finish(&mut cx, size);
    let code = cx.asm.finish().expect("every block is bound, and a function's jumps reach across it");
    Ok(Compiled { code, frame: size, marks: cx.marks })
}

impl<A: Isel> Cx<'_, A> {
    fn block(&mut self, block: Block) -> Result<(), Lack> {
        self.asm.bind(self.labels[block]);
        if Some(block) != self.f.entry() && self.kept.take() != Some(block) {
            self.clear();
        }
        let fused = self.fused(block).map(|fused| fused.value);
        for &inst in &self.f.blocks[block].insts {
            let data = &self.f.insts[inst];
            // The comparison the branch fuses is made by the branch.
            if fused.is_some() && data.result == fused {
                continue;
            }
            self.at = data.origin;
            self.marks.push((self.asm.here(), data.origin));
            vun_trace::tracing::trace!(inst = %data.kind, at = self.asm.here(), "select");
            // A value recomputed where used reads nothing where it is defined.
            if data.result.is_none_or(|v| self.values[v].recompute.is_none()) {
                A::inst(self, inst)?;
            }
            self.release(|| data.kind.operands(), data.result);
        }
        self.at = self.f.blocks[block].origin;
        self.marks.push((self.asm.here(), self.at));
        A::terminate(self, block)
    }
}

impl<A: Arch> Cx<'_, A> {
    /// The label of `block`.
    #[must_use]
    pub fn label(&self, block: Block) -> Label {
        self.labels[block]
    }

    /// The offset of `slot` from the frame pointer.
    #[must_use]
    pub fn slot(&self, slot: Slot) -> i32 {
        self.slots[slot]
    }

    /// What `missing` is lacking at the code compiled now.
    #[must_use]
    pub fn lack(&self, missing: Missing) -> Lack {
        missing.at(self.at)
    }
}
