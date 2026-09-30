//! Lowering one function: a local per LIR value, a frame in linear memory below
//! `__stack_pointer` for the slots and the copies calls pass, and the body the control
//! structure emits.

use vun_backend::{Lack, Missing};
use vun_index::IndexVec;
use vun_lir::{Ext, Function, Scalar, Slot, Value};

use super::Cx;
use super::abi::{Given, One, Part, Passing, narrow, val};
use crate::module::op::{Mem, Num, Op};
use crate::module::{Func, ValType};

/// The alignment of the stack pointer between calls, which a frame keeps.
const STACK_ALIGN: u64 = 16;

/// A function being lowered.
pub(super) struct Lower<'a> {
    pub(super) cx: &'a Cx<'a>,
    pub(super) f: &'a Function,
    pub(super) passing: &'a Passing,
    /// The wasm parameters' count; locals past them are `locals`.
    params: u32,
    locals: Vec<ValType>,
    /// The local holding each value.
    pub(super) of: IndexVec<Value, u32>,
    pub(super) body: Vec<Op>,
    frame: Frame,
    /// The address a result is written through, a parameter or a place in the frame.
    pub(super) out: Option<u32>,
    /// Where the code of each LIR instruction and terminator starts, with its origin.
    marks: Vec<(usize, vun_origin::OriginId)>,
}

/// The frame's layout, and where its size and alignment go once known.
struct Frame {
    size: u64,
    align: u64,
    slots: IndexVec<Slot, u64>,
    /// The local holding its base, and the one holding the stack pointer before it.
    base: u32,
    saved: u32,
    /// The places of its size and alignment in the body.
    patch: (usize, usize),
}

impl<'a> Lower<'a> {
    pub(super) fn new(cx: &'a Cx<'a>, f: &'a Function, passing: &'a Passing) -> Result<Self, Lack> {
        let at = f.origin;
        let params = u32::try_from(passing.ty.params.len()).expect("parameters fit u32");
        let mut lower = Self {
            cx,
            f,
            passing,
            params,
            locals: Vec::new(),
            of: IndexVec::new(),
            body: Vec::new(),
            frame: Frame {
                size: 0,
                align: STACK_ALIGN,
                slots: IndexVec::new(),
                base: 0,
                saved: 0,
                patch: (0, 0),
            },
            out: None,
            marks: Vec::new(),
        };
        for (_, v) in f.values.iter() {
            let held = val(v.ty).map_err(|m| m.at(at))?;
            let local = lower.temp(held);
            lower.of.push(local);
        }
        lower.prologue();
        lower.entry().map_err(|m| m.at(at))?;
        Ok(lower)
    }

    /// A new local of type `ty`.
    pub(super) fn temp(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        self.params + u32::try_from(self.locals.len() - 1).expect("locals fit u32")
    }

    /// Marks where the code of what `origin` names starts.
    pub(super) fn mark(&mut self, origin: vun_origin::OriginId) {
        self.marks.push((self.body.len(), origin));
    }

    pub(super) fn op(&mut self, op: Op) {
        self.body.push(op);
    }

    pub(super) fn get(&mut self, v: Value) {
        self.op(Op::LocalGet(self.of[v]));
    }

    pub(super) fn set(&mut self, v: Value) {
        self.op(Op::LocalSet(self.of[v]));
    }

    /// Lowers the stack pointer by the frame, aligned: its size and alignment are patched when
    /// the body is done.
    fn prologue(&mut self) {
        let sp = self.cx.stack_pointer;
        self.frame.saved = self.temp(ValType::I32);
        self.frame.base = self.temp(ValType::I32);
        self.op(Op::GlobalGet(sp));
        self.op(Op::LocalTee(self.frame.saved));
        self.frame.patch.0 = self.body.len();
        self.op(Op::I32Const(0));
        self.op(Op::Num(Num::I32Sub));
        self.frame.patch.1 = self.body.len();
        self.op(Op::I32Const(0));
        self.op(Op::Num(Num::I32And));
        self.op(Op::LocalTee(self.frame.base));
        self.op(Op::GlobalSet(sp));
        for (slot, data) in self.f.slots.iter() {
            let at = self.alloc(data.size, data.align);
            let pushed = self.frame.slots.push(at);
            debug_assert_eq!(pushed, slot);
        }
    }

    /// Room in the frame for `size` bytes aligned to `align`; its offset from the base.
    pub(super) fn alloc(&mut self, size: u64, align: u64) -> u64 {
        let at = self.frame.size.next_multiple_of(align.max(1));
        self.frame.size = at + size;
        self.frame.align = self.frame.align.max(align);
        at
    }

    /// The address `offset` bytes into the frame.
    pub(super) fn frame_addr(&mut self, offset: u64) {
        self.op(Op::LocalGet(self.frame.base));
        self.op(Op::I32Const(Self::i32(offset)));
        self.op(Op::Num(Num::I32Add));
    }

    pub(super) fn slot_addr(&mut self, slot: Slot) {
        self.frame_addr(self.frame.slots[slot]);
    }

    /// The entry block's parameters from the wasm parameters.
    fn entry(&mut self) -> Result<(), Missing> {
        let entry = self.f.entry().expect("a function has a body");
        let values = self.f.blocks[entry].params.clone();
        let parts = self.passing.parts.clone();
        for (wasm, (&v, part)) in (u32::from(self.passing.through())..).zip(values.iter().zip(&parts)) {
            match *part {
                Part::Value(_, narrowed) => {
                    self.op(Op::LocalGet(wasm));
                    if let Some((bits, Ext::Sign)) = narrowed {
                        self.mask(bits);
                    }
                }
                Part::Scalar(one) => {
                    let at = self.alloc(one.size, one.align);
                    self.frame_addr(at + one.offset);
                    self.op(Op::LocalGet(wasm));
                    self.op(Op::Store(Self::mem(one.ty)?));
                    self.frame_addr(at);
                }
                Part::Copy { .. } => self.op(Op::LocalGet(wasm)),
            }
            self.set(v);
        }
        if let Some(&v) = values.get(parts.len()) {
            if let Given::Scalar(one) = self.passing.given {
                let at = self.alloc(one.size, one.align);
                self.frame_addr(at);
            } else {
                self.op(Op::LocalGet(0));
            }
            self.set(v);
            self.out = Some(self.of[v]);
        }
        Ok(())
    }

    /// Restores the stack pointer and returns `result`, as the signature gives it.
    pub(super) fn ret(&mut self, result: Option<Value>) -> Result<(), Missing> {
        self.op(Op::LocalGet(self.frame.saved));
        self.op(Op::GlobalSet(self.cx.stack_pointer));
        match (self.passing.given, result) {
            (Given::Value(_, narrowed), Some(v)) => {
                self.get(v);
                if let Some((bits, Ext::Sign)) = narrowed {
                    self.extend(bits);
                }
            }
            (Given::Scalar(one), _) => {
                let out = self.out.expect("a result in memory has an address");
                self.op(Op::LocalGet(out));
                self.load_at(one)?;
            }
            _ => {}
        }
        self.op(Op::Return);
        Ok(())
    }

    /// Loads the scalar of a record of one at the address on the stack.
    pub(super) fn load_at(&mut self, one: One) -> Result<(), Missing> {
        self.op(Op::I32Const(Self::i32(one.offset)));
        self.op(Op::Num(Num::I32Add));
        self.op(Op::Load(Self::mem(one.ty)?));
        Ok(())
    }

    /// Keeps the low `bits` of the `i32` on the stack, as a narrow integer is held.
    pub(super) fn mask(&mut self, bits: u32) {
        self.op(Op::I32Const(i32::from_ne_bytes(((1_u32 << bits) - 1).to_ne_bytes())));
        self.op(Op::Num(Num::I32And));
    }

    /// Extends the sign of the narrow integer on the stack to 32 bits.
    pub(super) fn extend(&mut self, bits: u32) {
        self.op(Op::Num(if bits == 8 { Num::I32Extend8S } else { Num::I32Extend16S }));
    }

    /// How a scalar of type `ty` is loaded and stored: a narrow integer as its bytes.
    pub(super) fn mem(ty: Scalar) -> Result<Mem, Missing> {
        let bytes = u8::try_from(narrow(ty).map_or_else(|| ty.bytes(32), |bits| u64::from(bits / 8)))
            .expect("scalars are at most 8 bytes");
        Ok(Mem { ty: val(ty)?, bytes })
    }

    /// An offset or size as the `i32` immediate an address takes.
    pub(super) fn i32(n: u64) -> i32 {
        i32::try_from(n).expect("offsets of wasm32 fit i32")
    }

    /// The function, once its body is emitted.
    pub(super) fn finish(mut self) -> Func {
        let size = self.frame.size.next_multiple_of(self.frame.align);
        self.body[self.frame.patch.0] = Op::I32Const(Self::i32(size));
        self.body[self.frame.patch.1] = Op::I32Const(-Self::i32(self.frame.align));
        self.op(Op::Unreachable);
        let ty = self.cx.module.type_of(&self.passing.ty).expect("signatures are listed first");
        Func { ty, locals: self.locals, body: self.body, marks: self.marks }
    }
}
