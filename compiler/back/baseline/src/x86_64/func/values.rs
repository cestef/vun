//! Where values are: registers, homes, and values recomputed where used.

use vun_lir::func::Def;
use vun_lir::{Address, InstKind, Value};

use super::super::{Base, Mem, Reg, Size, Via};
use super::{FnCompiler, HOME};

impl FnCompiler<'_> {
    /// What defines `v`, when an instruction does.
    fn defined(&self, v: Value) -> Option<&InstKind> {
        match self.f.values[v].def {
            Def::Inst(inst) => Some(&self.f.insts[inst].kind),
            Def::Param(..) => None,
        }
    }

    /// Whether `v` is recomputed where used rather than kept: a constant, or an address in the
    /// frame or of something the linker places, possibly at a constant offset.
    pub(super) fn remat(&self, v: Value) -> bool {
        self.frame_addr.contains_key(&v) || self.defined(v).is_some_and(|k| self.remat_kind(k))
    }

    fn remat_kind(&self, kind: &InstKind) -> bool {
        match kind {
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => true,
            InstKind::Offset { base, offset } => {
                self.address(*base).is_some() && self.constant(*offset).is_some()
            }
            _ => false,
        }
    }

    /// The constant `v` is, as a 32-bit displacement or immediate, when it is one that fits.
    pub(super) fn constant(&self, v: Value) -> Option<i32> {
        let Some(InstKind::Const { ty, bits }) = self.defined(v) else { return None };
        let size = Size::of(*ty)?;
        let bits = u64::try_from(*bits).ok()?;
        // Sign-extended from the constant's width, as an immediate is.
        let shift = 64 - size.bytes() * 8;
        i32::try_from((bits << shift).cast_signed() >> shift).ok()
    }

    /// The memory `v` addresses when it is recomputed: `None` for anything else, and for an
    /// address reached through the global offset table.
    pub(super) fn address(&self, v: Value) -> Option<Mem> {
        if let Some(&at) = self.frame_addr.get(&v) {
            return Some(Mem::at(Reg::Rbp, at));
        }
        match self.defined(v)? {
            InstKind::SlotAddr(s) => Some(Mem::at(Reg::Rbp, self.slots[*s])),
            InstKind::Addr(a @ (Address::Func(_) | Address::Data(_))) => {
                Some(Mem { base: Base::Rip(*a, Via::Direct), disp: 0 })
            }
            InstKind::Offset { base, offset } => Some(self.address(*base)?.plus(self.constant(*offset)?)),
            _ => None,
        }
    }

    /// Writes the recomputed `v` into `dst`.
    fn materialize(&mut self, v: Value, dst: Reg) {
        if let Some(mem) = self.address(v) {
            self.asm.lea(dst, mem);
            return;
        }
        match self.defined(v) {
            Some(&InstKind::Const { bits, .. }) => {
                self.asm.mov_ri(dst, u64::try_from(bits & u128::from(u64::MAX)).expect("masked"));
            }
            Some(&InstKind::Addr(a)) => {
                self.asm.load(Size::B64, dst, Mem { base: Base::Rip(a, Via::Got), disp: 0 });
            }
            _ => unreachable!("only recomputed values are materialized"),
        }
    }

    /// The home of `v`, allocated when first needed.
    pub(super) fn home(&mut self, v: Value) -> Mem {
        let at = if let Some(at) = self.home[v] {
            at
        } else {
            let at = self.reserve(HOME, HOME);
            self.home[v] = Some(at);
            at
        };
        Mem::at(Reg::Rbp, at)
    }

    pub(super) fn bind(&mut self, v: Value, r: Reg, saved: bool) {
        // A value with a register of its own is moved there.
        if let Some(fixed) = self.fixed[v]
            && fixed != r
        {
            self.asm.mov_rr(fixed, r);
            return self.bind(v, fixed, saved);
        }
        self.holds[r.number() as usize] = Some(v);
        self.reg[v] = Some(r);
        self.saved[v] = saved;
    }

    fn unbind(&mut self, r: Reg) {
        if let Some(v) = self.holds[r.number() as usize].take() {
            self.reg[v] = None;
        }
    }

    /// Frees `r`, storing what it holds home if it is still used there only.
    pub(super) fn evict(&mut self, r: Reg) {
        if let Some(v) = self.holds[r.number() as usize] {
            self.store_home(v, r);
        }
        self.unbind(r);
    }

    /// Stores `v`, held in `r`, home when it is used later and its home does not hold it.
    fn store_home(&mut self, v: Value, r: Reg) {
        if !self.remat(v) && !self.saved[v] && self.left[v] > 0 && self.fixed[v].is_none() {
            let home = self.home(v);
            self.asm.store(Size::B64, home, r);
            self.saved[v] = true;
        }
    }

    /// Frees `r` for an instruction that writes it; what it held is stored home first.
    pub(super) fn take(&mut self, r: Reg) {
        self.evict(r);
    }

    /// A free register, evicting one if none is.
    fn alloc(&mut self) -> Reg {
        let free = Reg::ALLOCATABLE
            .into_iter()
            .find(|r| self.holds[r.number() as usize].is_none() && !self.locked.contains(r));
        if let Some(r) = free {
            return r;
        }
        loop {
            let r = Reg::ALLOCATABLE[self.victim % Reg::ALLOCATABLE.len()];
            self.victim += 1;
            if !self.locked.contains(&r) {
                self.evict(r);
                return r;
            }
        }
    }

    /// The register holding `v` for the instruction compiled, which keeps it there.
    pub(super) fn get(&mut self, v: Value) -> Reg {
        let r = if let Some(r) = self.reg[v] {
            r
        } else {
            let r = self.alloc();
            self.get_into(v, r);
            let saved = !self.remat(v);
            self.bind(v, r, saved);
            r
        };
        self.locked.push(r);
        r
    }

    /// Writes `v` into `dst`, which does not then hold it for later uses.
    pub(super) fn get_into(&mut self, v: Value, dst: Reg) {
        if let Some(r) = self.reg[v] {
            self.asm.mov_rr(dst, r);
        } else if self.remat(v) {
            self.materialize(v, dst);
        } else {
            let home = self.home(v);
            self.asm.load(Size::B64, dst, home);
        }
    }

    /// A register for the value `v` defines.
    pub(super) fn def(&mut self, v: Value) -> Reg {
        let r = self.fixed[v].unwrap_or_else(|| self.alloc());
        self.bind(v, r, false);
        self.locked.push(r);
        r
    }

    /// After an instruction: one use of each operand is done, and a value used no more frees
    /// its register.
    pub(super) fn release(&mut self, operands: &[Value], result: Option<Value>) {
        for &v in operands {
            self.left[v] = self.left[v].saturating_sub(1);
        }
        for v in operands.iter().copied().chain(result) {
            if self.left[v] == 0
                && let Some(r) = self.reg[v]
            {
                self.unbind(r);
            }
        }
        self.locked.clear();
    }

    /// Stores home every value still used whose home does not hold it.
    pub(super) fn spill_live(&mut self) {
        for r in Reg::ALL {
            if let Some(v) = self.holds[r.number() as usize] {
                self.store_home(v, r);
            }
        }
    }

    /// Forgets what every register holds, but the values held in registers of their own.
    pub(super) fn clear(&mut self) {
        for r in Reg::ALL {
            if !self.is_fixed(r) {
                self.unbind(r);
            }
        }
    }
}
