//! C records by value: loaded into the registers they travel in and stored back from them, copied
//! where the convention has the caller copy them, and the address a result in memory comes back
//! through (`plan`).

use cranelift_codegen::ir::{InstBuilder, StackSlotData, StackSlotKind, Value as Val, types};
use vun_lir::Value;

use super::Lower;
use crate::abi::Plan;
use crate::plan::{Lane, Layout, Param, Parts, Pass, Returned, SLOT, Skip};

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
        for Param { skips, pass } in &plan.params {
            let structs = matches!(pass, Pass::Struct(_));
            for skip in skips {
                passed.push(self.dummy(*skip, structs));
            }
            match pass {
                Pass::Values(count) => passed.extend(values.by_ref().take(*count)),
                Pass::Parts(parts) => {
                    let at = values.next().expect("a record has an address");
                    passed.extend(self.load_parts(parts, at));
                }
                Pass::Struct(layout) if layout.size == layout.words() => passed.extend(values.next()),
                Pass::Struct(layout) => {
                    // A struct argument is read whole words at a time: from a copy of its own.
                    let from = values.next().expect("a record has an address");
                    let copy = self.scratch(Layout { size: layout.words(), align: SLOT });
                    self.copy(copy, from, layout.size);
                    passed.push(copy);
                }
                Pass::Words(layout) => {
                    let at = values.next().expect("a record has an address");
                    for lane in Self::words(*layout) {
                        passed.push(self.load_lane(&lane, at));
                    }
                }
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
        parts.lanes.iter().map(|lane| self.load_lane(lane, at)).collect()
    }

    /// Stores `values`, the parts of a record, at `at`.
    pub(super) fn store_parts(&mut self, parts: &Parts, at: Val, values: &[Val]) {
        for (lane, &value) in parts.lanes.iter().zip(values) {
            self.store_lane(lane, at, value);
        }
    }

    /// The words of a record of `layout`, the last as many bytes as are left, each as an
    /// integer.
    fn words(layout: Layout) -> impl Iterator<Item = Lane> {
        (0..layout.words() / SLOT).map(move |word| {
            let offset = word * SLOT;
            Lane { offset, bytes: (layout.size - offset).min(SLOT), ty: types::I64 }
        })
    }

    /// The bytes of `lane` at `at` as the integer or float it is held in: where they are fewer
    /// than its type's, read in the widest pieces that fit, little-endian, with zeros above.
    fn load_lane(&mut self, lane: &Lane, at: Val) -> Val {
        let flags = Self::plain();
        let offset = |o: u64| i32::try_from(lane.offset + o).expect("records are within a signed offset");
        if lane.bytes == u64::from(lane.ty.bytes()) {
            return self.b.ins().load(lane.ty, flags, at, offset(0));
        }
        let mut value = None;
        for (piece, bytes) in Self::pieces(lane.bytes) {
            let part = match bytes {
                1 => self.b.ins().uload8(lane.ty, flags, at, offset(piece)),
                2 => self.b.ins().uload16(lane.ty, flags, at, offset(piece)),
                _ => self.b.ins().uload32(flags, at, offset(piece)),
            };
            let part = if piece == 0 {
                part
            } else {
                self.b.ins().ishl_imm_u(part, i64::try_from(piece * 8).expect("a shift"))
            };
            value = Some(value.map_or(part, |v| self.b.ins().bor(v, part)));
        }
        value.expect("a lane holds a byte")
    }

    /// Stores the bytes of `value`, the lane's, at `at`, in the pieces `load_lane` reads.
    fn store_lane(&mut self, lane: &Lane, at: Val, value: Val) {
        let flags = Self::plain();
        let offset = |o: u64| i32::try_from(lane.offset + o).expect("records are within a signed offset");
        if lane.bytes == u64::from(lane.ty.bytes()) {
            self.b.ins().store(flags, value, at, offset(0));
            return;
        }
        for (piece, bytes) in Self::pieces(lane.bytes) {
            let part = if piece == 0 {
                value
            } else {
                self.b.ins().ushr_imm_u(value, i64::try_from(piece * 8).expect("a shift"))
            };
            match bytes {
                1 => self.b.ins().istore8(flags, part, at, offset(piece)),
                2 => self.b.ins().istore16(flags, part, at, offset(piece)),
                _ => self.b.ins().istore32(flags, part, at, offset(piece)),
            };
        }
    }

    /// The pieces of `bytes` bytes, each a power of two at most 4 bytes, from the first: its
    /// offset and size.
    fn pieces(bytes: u64) -> Vec<(u64, u64)> {
        let (mut offset, mut pieces) = (0, Vec::new());
        while offset < bytes {
            let size = [4, 2, 1].into_iter().find(|&s| s <= bytes - offset).expect("a piece of a byte");
            pieces.push((offset, size));
            offset += size;
        }
        pieces
    }

    /// The value a parameter carrying nothing is passed.
    fn dummy(&mut self, skip: Skip, structs: bool) -> Val {
        match skip {
            Skip::Float => self.b.ins().f64const(0.0),
            Skip::Stack if structs => self.scratch(Layout { size: SLOT, align: SLOT }),
            Skip::Int | Skip::Stack => self.b.ins().iconst(types::I64, 0),
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
        for Param { skips, pass } in &plan.params {
            passed.by_ref().take(skips.len()).for_each(drop);
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
                Pass::Words(layout) => {
                    let at = self.scratch(*layout);
                    for lane in Self::words(*layout) {
                        let word = passed.next().expect("a word per eight bytes");
                        self.store_lane(&lane, at, word);
                    }
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
