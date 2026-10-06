//! The compilation of one function: the state the framework keeps as it walks the blocks, and
//! the walk. Where each value lives is planned first (`vun_regalloc`, spec 13, section 8.6);
//! the walk follows the plan, running its moves before and after each point and on each edge.

mod call;
mod compiled;
mod describe;
mod exit;
mod moves;
mod saves;
mod values;

pub use compiled::Compiled;
pub use exit::{Fused, Layout};

use vun_asm::{Isa, Label};
use vun_backend::{Lack, Missing};
use vun_dwarf::Step;
use vun_index::{Idx, IndexVec};
use vun_lir::analysis::{Cfg, Loops, Order};
use vun_lir::{Address, Block, Conv, Function, Slot, Unit, Value};
use vun_origin::OriginId;
use vun_regalloc::{Allocation, Machine, PReg, Remade};

use crate::frame::Frame;
use crate::values::Assignment;
use crate::{Arch, Code, Isel, Processor};

use describe::Described;

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
    values: IndexVec<Value, Assignment>,
    slots: IndexVec<Slot, i32>,
    /// The registers values are held in, and those a call preserves among them.
    machine: Machine,
    /// Each register of the machine by its index.
    regs: Vec<Option<A::Reg>>,
    /// Where each value lives; planned once the prologue has placed the parameters passed in
    /// memory.
    plan: Option<Allocation>,
    /// The block and the point compiled now, the scratch registers of the point taken, and the
    /// values recomputed into them.
    block: Block,
    point: u32,
    temps: usize,
    remade: Vec<(Value, A::Reg)>,
    /// The callee-saved registers the plan uses, with where the prologue saved each.
    saves: Vec<(A::Reg, i32)>,
    /// Where each instruction's code starts, with its origin.
    marks: Vec<(u64, OriginId)>,
    /// The steps of the frame, where each takes effect.
    unwind: Vec<(u64, Step)>,
    /// Which values the branch ending their block makes (`Cx::is_fused`), found when first asked.
    fusing: std::cell::OnceCell<Vec<bool>>,
}

/// Compiles `f` of `unit`, listing its code when `listed`.
///
/// # Errors
/// Returns what the backend cannot compile.
///
/// # Panics
/// Never for a function whose frame and code fit the architecture's offsets; in debug builds,
/// when the register allocator's plan fails its check, a bug of the allocator or of what an
/// architecture says its instructions need.
pub fn compile<A: Isel>(
    unit: &Unit,
    f: &Function,
    convention: A::Convention,
    processor: Processor<A>,
    listed: bool,
) -> Result<Compiled<<A::Isa as Isa>::Inst<Address>>, Lack> {
    // A naked function's code is its blocks as written: no frame is set up, saved or left.
    let naked = f.signature.conv == Conv::Naked;
    let cfg = Cfg::of(f);
    let loops = Loops::of(f, &cfg);
    let order = Order::of(f, &loops);
    let mut asm = if listed { Code::<A>::listed(processor) } else { Code::<A>::new(processor) };
    let labels = f.blocks.iter().map(|_| asm.label()).collect::<Vec<_>>().into();
    let mut frame = Frame::default();
    let mut slots = IndexVec::new();
    for (_, slot) in f.slots.iter() {
        if slot.align > A::STACK_ALIGN {
            return Err(Missing::Align(slot.align).at(slot.origin));
        }
        slots.push(frame.reserve(slot.size, slot.align));
    }
    let values = Assignment::all::<A>(f, &loops, &order, &slots, unit.pointer_bits);
    let (machine, regs) = Cx::<A>::machine(A::preserved(convention));
    let mut cx = Cx {
        unit,
        f,
        state: A::state(f, convention)?,
        order,
        cfg,
        asm,
        frame,
        at: f.origin,
        labels,
        values,
        slots,
        machine,
        regs,
        plan: None,
        block: f.entry().unwrap_or(Block::new(0)),
        point: 0,
        temps: 0,
        remade: Vec::new(),
        saves: Vec::new(),
        // The prologue comes from the function itself.
        marks: vec![(0, f.origin)],
        unwind: Vec::new(),
        fusing: std::cell::OnceCell::new(),
    };
    if !naked {
        A::prologue(&mut cx)?;
        cx.emitted()?;
    }
    cx.lacking()?;
    cx.plan = Some(Described::plan(&cx));
    if !naked {
        cx.save();
    }
    for position in 0..cx.order.blocks().len() {
        let block = cx.order.blocks()[position];
        cx.block(block)?;
    }
    let size = cx.frame.size(A::STACK_ALIGN);
    if !naked {
        A::finish(&mut cx, size);
        cx.emitted()?;
    }
    let code = cx.asm.finish().expect("every block is bound, and a function's jumps reach across it");
    Ok(Compiled { code, frame: size, marks: cx.marks, unwind: cx.unwind })
}

impl<A: Isel> Cx<'_, A> {
    /// The first thing an instruction needs that the backend lacks; a value made where used is
    /// no instruction compiled.
    fn lacking(&self) -> Result<(), Lack> {
        for (_, data) in self.f.blocks.iter() {
            for &inst in &data.insts {
                if self.f.insts[inst].result.is_some_and(|v| self.values[v].made.is_some()) {
                    continue;
                }
                A::lacks(self, inst).inspect_err(|lack| {
                    vun_trace::tracing::debug!(inst = %self.f.insts[inst].kind, ?lack, "lacks");
                })?;
            }
        }
        Ok(())
    }

    fn block(&mut self, block: Block) -> Result<(), Lack> {
        self.asm.bind(self.labels[block]);
        self.block = block;
        let (first, last) = self.plan().points.blocks[block].expect("an emitted block");
        if Some(block) == self.f.entry() {
            self.enter(first);
            A::entry(self)?;
            self.emitted()?;
            self.leave_point(first);
        }
        let fused = self.fused(block).map(|fused| fused.value);
        for &inst in &self.f.blocks[block].insts {
            let data = &self.f.insts[inst];
            self.at = data.origin;
            self.marks.push((self.asm.here(), data.origin));
            vun_trace::tracing::trace!(inst = %data.kind, at = self.asm.here(), "select");
            self.enter(self.plan().points.insts[inst].expect("a numbered instruction"));
            // The comparison the branch fuses is made by the branch; a value made where used
            // reads nothing where it is defined, and one held is made into its register.
            match data.result.and_then(|v| Some((v, self.values[v].made?))) {
                Some((_, (_, Remade::Used))) => {}
                Some((v, (what, Remade::Held))) => {
                    let dst = self.def(v);
                    A::recompute(&mut self.asm, dst, what);
                }
                None if fused.is_some() && data.result == fused => {}
                None => A::inst(self, inst)?,
            }
            self.emitted()?;
            self.leave_point(self.point);
        }
        self.at = self.f.blocks[block].origin;
        self.marks.push((self.asm.here(), self.at));
        self.enter(last);
        vun_trace::tracing::trace!(term = %self.f.blocks[block].term, at = self.asm.here(), "terminate");
        A::terminate(self, block)?;
        self.emitted()
    }

    /// What the processor lacks of the instructions emitted since last asked, which the backend
    /// then lacks where the code comes from.
    fn emitted(&mut self) -> Result<(), Lack> {
        self.asm.lacked().map_or(Ok(()), |needed| Err(Missing::Unavailable(needed).at(self.at)))
    }

    /// Starts the point `point`: its moves run, and none of its scratch registers is taken.
    fn enter(&mut self, point: u32) {
        self.point = point;
        self.temps = 0;
        self.remade.clear();
        let moves = self.plan().before(point).to_vec();
        self.run(&moves);
    }

    /// Ends the point `point`: the moves after it run.
    fn leave_point(&mut self, point: u32) {
        let moves = self.plan().after(point).to_vec();
        self.run(&moves);
    }
}

impl<A: Arch> Cx<'_, A> {
    /// The machine as the allocator sees it, from the registers the architecture holds values
    /// in and those `preserved` by calls but the frame pointer, and each register by its index.
    fn machine(preserved: &[A::Reg]) -> (Machine, Vec<Option<A::Reg>>) {
        let kept: Vec<A::Reg> = preserved.iter().copied().filter(|&r| r != A::FRAME).collect();
        let mut regs = vec![None; A::REGS];
        let general: Vec<PReg> = A::ALLOCATABLE
            .iter()
            .chain(&kept)
            .map(|&r| {
                regs[A::index(r)] = Some(r);
                A::preg(r)
            })
            .collect();
        let preserved = vun_regalloc::RegSet::of(&kept.iter().map(|&r| A::preg(r)).collect::<Vec<_>>());
        (Machine { general, float: Vec::new(), preserved }, regs)
    }

    /// The plan, once made.
    fn plan(&self) -> &Allocation {
        self.plan.as_ref().expect("planned after the prologue")
    }

    /// The machine register `p` is.
    fn reg_of(&self, p: PReg) -> A::Reg {
        self.regs[p.index()].expect("the plan names the machine's registers")
    }

    /// Records that the frame takes `step` here, after the code emitted so far.
    pub fn unwound(&mut self, step: Step) {
        self.unwind.push((self.asm.here(), step));
    }

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
