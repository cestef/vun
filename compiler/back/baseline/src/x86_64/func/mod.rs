//! The single-pass function compiler (spec 13, section 8.3): blocks in `Order`, instructions in
//! program order, each selected, given registers and encoded at once.
//!
//! Every value has an assignment: the register holding it, if any, and its home in the frame,
//! allocated when first needed, with whether the home holds it. A use decrements its remaining
//! uses; at zero the register is free. Constants and addresses of slots and data are
//! recomputed where used, never kept. Only the state at the current point is tracked: at the
//! end of a block, and before a call, every value still used is stored home, so each block
//! starts with every value in its home and every register free, but a block emitted next that
//! only the last one reaches, which starts with the registers as it left them. A value live
//! across a loop may instead hold a callee-saved register of its own (`fixed`).

mod call;
mod fixed;
mod inst;
mod memory;
mod term;
mod values;

use std::collections::HashMap;

use vun_backend::{Lack, Missing};
use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Liveness, Loops, Order};
use vun_lir::{Block, Conv, Function, Inst, Slot, Unit, Value};
use vun_origin::OriginId;

use super::abi::{Frame, Loc, Ret};
use super::encode::{Asm, CodeReloc, Label};
use super::{Mem, Reg};

/// A function's machine code.
pub struct Compiled {
    pub code: Vec<u8>,
    pub relocs: Vec<CodeReloc>,
    /// The bytes its frame takes below the return address and the saved frame pointer.
    pub frame: u64,
}

impl Compiled {
    /// The bytes of stack a call of the function takes: its frame, the return address and the
    /// saved frame pointer, as C compilers count them.
    #[must_use]
    pub fn stack(&self) -> u64 {
        self.frame + u64::from(LINKAGE)
    }
}

/// Bytes a home takes: a register's.
const HOME: u64 = 8;
/// Bytes a call pushes above a frame: the return address and the saved frame pointer.
const LINKAGE: u32 = 16;
/// The alignment of the stack at a call.
const STACK_ALIGN: u64 = 16;
/// Where the caller's stack arguments start, above the saved frame pointer and return address.
const CALLER_ARGS: i32 = LINKAGE.cast_signed();

struct FnCompiler<'a> {
    unit: &'a Unit,
    f: &'a Function,
    order: Order,
    cfg: Cfg,
    asm: Asm,
    labels: IndexVec<Block, Label>,
    /// What each register holds, by number.
    holds: [Option<Value>; 16],
    reg: IndexVec<Value, Option<Reg>>,
    home: IndexVec<Value, Option<i32>>,
    /// Whether the home holds the value.
    saved: IndexVec<Value, bool>,
    left: IndexVec<Value, u32>,
    /// Values that are addresses in the frame, recomputed where used: parameters passed by
    /// address, and where a result in registers is gathered.
    frame_addr: HashMap<Value, i32>,
    /// The registers holding operands of the instruction compiled, which nothing evicts.
    locked: Vec<Reg>,
    /// The next register evicted when none is free.
    victim: usize,
    slots: IndexVec<Slot, i32>,
    /// Bytes of the frame below the frame pointer taken so far.
    depth: u64,
    /// The most bytes a call's stack arguments take.
    outgoing: u64,
    abi: Frame,
    /// The home of the address a result is written through, which is given back.
    hidden: Option<i32>,
    /// Where a result returned in registers is gathered.
    gathered: Option<i32>,
    /// Where the instruction compiled comes from.
    at: OriginId,
    /// The block emitted next when it starts with the registers as the last one left them.
    kept: Option<Block>,
    /// The callee-saved register each value live across a loop is held in, if it has one.
    fixed: IndexVec<Value, Option<Reg>>,
    /// The callee-saved registers taken, with where the prologue saved each.
    saves: Vec<(Reg, i32)>,
}

/// Compiles `f` of `unit`.
///
/// # Errors
/// Returns what the backend cannot compile yet.
///
/// # Panics
/// Never for a function whose frame fits the 32-bit offsets x86-64 addresses it with.
pub fn compile(unit: &Unit, f: &Function) -> Result<Compiled, Lack> {
    if f.signature.conv == Conv::Naked {
        return Err(Missing::Naked.at(f.origin));
    }
    let cfg = Cfg::of(f);
    let loops = Loops::of(f, &cfg);
    let order = Order::of(f, &loops);
    let live = Liveness::of(f, &loops, &order);
    let abi = Frame::of(&f.signature).map_err(|m| m.at(f.origin))?;
    let mut asm = Asm::default();
    let labels = f.blocks.iter().map(|_| asm.label()).collect::<Vec<_>>().into();
    let values = f.values.len();
    let mut c = FnCompiler {
        unit,
        f,
        order,
        cfg,
        asm,
        labels,
        holds: [None; 16],
        reg: IndexVec::filled(None, values),
        home: IndexVec::filled(None, values),
        saved: IndexVec::filled(false, values),
        left: f.values.iter().map(|(v, _)| live.range(v).map_or(0, |r| r.uses)).collect::<Vec<_>>().into(),
        frame_addr: HashMap::new(),
        locked: Vec::new(),
        victim: 0,
        slots: IndexVec::new(),
        depth: 0,
        outgoing: 0,
        abi,
        hidden: None,
        gathered: None,
        at: f.origin,
        kept: None,
        fixed: IndexVec::filled(None, values),
        saves: Vec::new(),
    };
    let size_at = c.asm.prologue();
    for (_, slot) in f.slots.iter() {
        if slot.align > STACK_ALIGN {
            return Err(Missing::Align(slot.align).at(slot.origin));
        }
        let offset = c.reserve(slot.size, slot.align);
        c.slots.push(offset);
    }
    c.fix(&live, &loops);
    c.entry();
    for &block in &c.order.blocks().to_vec() {
        c.block(block)?;
    }
    c.asm.finish();
    let frame = (c.depth + c.outgoing).next_multiple_of(STACK_ALIGN);
    c.asm.patch32(size_at, u32::try_from(frame).expect("frames fit 4 GiB"));
    Ok(Compiled { code: c.asm.code, relocs: c.asm.relocs, frame })
}

impl FnCompiler<'_> {
    /// Reserves `size` bytes of the frame at `align`: their offset from the frame pointer.
    fn reserve(&mut self, size: u64, align: u64) -> i32 {
        self.depth = (self.depth + size).next_multiple_of(align.max(1));
        -i32::try_from(self.depth).expect("frames fit 2 GiB")
    }

    /// Receives the signature's values where the convention passes them.
    fn entry(&mut self) {
        let Some(entry) = self.f.entry() else { return };
        let params = self.f.blocks[entry].params.clone();
        let locs = self.abi.params.clone();
        for (&v, loc) in params.iter().zip(&locs) {
            match loc {
                Loc::Reg(r) => self.bind(v, *r, false),
                Loc::Stack(offset) => {
                    self.home[v] = Some(CALLER_ARGS + i32::try_from(*offset).expect("fits"));
                    self.saved[v] = true;
                    if let Some(r) = self.fixed[v] {
                        let home = self.home(v);
                        self.asm.load(super::Size::B64, r, home);
                    }
                }
                Loc::Copied { offset, .. } => {
                    self.frame_addr.insert(v, CALLER_ARGS + i32::try_from(*offset).expect("fits"));
                }
                Loc::Split(parts) => {
                    let buffer = self.reserve(parts.len() as u64 * HOME, HOME);
                    for (i, &(r, _)) in parts.iter().enumerate() {
                        let at = Mem::at(Reg::Rbp, buffer + i32::try_from(i as u64 * HOME).expect("fits"));
                        self.asm.store(super::Size::B64, at, r);
                    }
                    self.frame_addr.insert(v, buffer);
                }
                Loc::Returned => {
                    let Ret::Split(parts) = &self.abi.ret else { continue };
                    let buffer = self.reserve(parts.len() as u64 * HOME, HOME);
                    self.frame_addr.insert(v, buffer);
                    self.gathered = Some(buffer);
                }
            }
        }
        if self.abi.ret == Ret::Hidden {
            let home = self.reserve(HOME, HOME);
            self.asm.store(super::Size::B64, Mem::at(Reg::Rbp, home), super::abi::ARGS[0]);
            self.hidden = Some(home);
        }
        // An entry that is also jumped to receives its parameters in their homes.
        if !self.cfg.preds[entry].is_empty() {
            self.spill_live();
            self.clear();
        }
    }

    fn block(&mut self, block: Block) -> Result<(), Lack> {
        let label = self.labels[block];
        self.asm.bind(label);
        if Some(block) != self.f.entry() && self.kept.take() != Some(block) {
            self.clear();
        }
        let fused = self.fused(block).map(|(v, ..)| v);
        for &inst in &self.f.blocks[block].insts {
            // The comparison the branch fuses is made by the branch.
            if fused.is_some() && self.f.insts[inst].result == fused {
                continue;
            }
            self.inst(inst)?;
        }
        self.at = self.f.blocks[block].origin;
        self.terminate(block)
    }

    fn inst(&mut self, inst: Inst) -> Result<(), Lack> {
        let data = &self.f.insts[inst];
        self.at = data.origin;
        if data.result.is_some_and(|v| self.remat(v)) {
            return Ok(());
        }
        self.select(&data.kind, data.result)?;
        let operands: Vec<Value> = data.kind.operands().collect();
        self.release(&operands, data.result);
        Ok(())
    }

    fn lack(&self, missing: Missing) -> Lack {
        missing.at(self.at)
    }
}
