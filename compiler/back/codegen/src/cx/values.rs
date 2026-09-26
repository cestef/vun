//! Where values are, as instruction compilers ask: operands brought into registers, results
//! given registers, a dying operand's register taken over by the result, scratch registers, and
//! registers freed for instructions that write fixed ones. Registers an instruction uses are
//! locked until it is done, so nothing it holds is evicted under it.

use vun_asm::imm::sign_extend;
use vun_lir::Value;

use super::Cx;
use crate::Arch;
use crate::values::{Place, Recompute};

impl<A: Arch> Cx<'_, A> {
    /// How `v` is recomputed where used, if it is.
    #[must_use]
    pub fn recomputed(&self, v: Value) -> Option<Recompute> {
        self.values[v].recompute
    }

    /// The constant `v` is, sign-extended from its width, when it is one of at most 64 bits.
    ///
    /// # Panics
    /// Never: the bits are masked to 64.
    #[must_use]
    pub fn imm(&self, v: Value) -> Option<i64> {
        let Some(Recompute::Const(bits)) = self.values[v].recompute else { return None };
        let width = self.f.ty(v).bits(self.unit.pointer_bits);
        (width <= 64).then(|| sign_extend(u64::try_from(bits & u128::from(u64::MAX)).expect("masked"), width))
    }

    /// The address `v` is, when it is known while compiling.
    #[must_use]
    pub fn place(&self, v: Value) -> Option<Place> {
        match self.values[v].recompute {
            Some(Recompute::Addr(place)) => Some(place),
            _ => None,
        }
    }

    /// The register holding `v` now, if one does.
    #[must_use]
    pub fn held(&self, v: Value) -> Option<A::Reg> {
        self.values[v].reg
    }

    /// The offset of the home holding `v`, when no register does.
    #[must_use]
    pub fn in_home(&self, v: Value) -> Option<i32> {
        let a = &self.values[v];
        (a.reg.is_none() && a.saved).then_some(a.home).flatten()
    }

    /// Whether the use compiled now is `v`'s last, so its register is free to overwrite.
    #[must_use]
    pub fn dies(&self, v: Value) -> bool {
        self.values[v].left == 1 && self.values[v].fixed.is_none()
    }

    /// Makes `v` recomputed as the address `place`: a parameter passed in memory.
    pub fn recompute_as(&mut self, v: Value, place: Place) {
        self.values[v].recompute = Some(Recompute::Addr(place));
    }

    /// Makes `v` live in memory at `offset` from the frame pointer, which holds it already.
    pub fn home_at(&mut self, v: Value, offset: i32) {
        self.values[v].home = Some(offset);
        self.values[v].saved = true;
        if let Some(r) = self.values[v].fixed {
            A::load(&mut self.asm, r, offset);
        }
    }

    /// The offset of `v`'s home, allocated when first needed.
    pub fn home(&mut self, v: Value) -> i32 {
        if let Some(at) = self.values[v].home {
            return at;
        }
        let at = self.frame.reserve(A::WORD, A::WORD);
        self.values[v].home = Some(at);
        at
    }

    /// The register holding `v` for the instruction compiled, brought there if it is not.
    pub fn reg(&mut self, v: Value) -> A::Reg {
        let r = if let Some(r) = self.values[v].reg {
            r
        } else {
            let r = self.alloc();
            self.reg_to(v, r);
            let saved = self.values[v].recompute.is_none();
            self.bind(v, r, saved);
            r
        };
        self.regs.lock(r);
        r
    }

    /// Writes `v` into `dst`, which does not then hold it for later uses.
    pub fn reg_to(&mut self, v: Value, dst: A::Reg) {
        let a = self.values[v];
        match (a.reg, a.recompute) {
            (Some(r), _) => A::copy(&mut self.asm, dst, r),
            (None, Some(what)) => A::recompute(&mut self.asm, dst, what),
            (None, None) => {
                let home = self.home(v);
                A::load(&mut self.asm, dst, home);
            }
        }
    }

    /// A register for the value `v` defines.
    pub fn def(&mut self, v: Value) -> A::Reg {
        let r = match self.values[v].fixed {
            Some(r) => r,
            None => self.alloc(),
        };
        self.bind(v, r, false);
        self.regs.lock(r);
        r
    }

    /// Makes `r`, which an instruction wrote, hold the value `v` defines.
    pub fn def_at(&mut self, v: Value, r: A::Reg) {
        self.bind(v, r, false);
        self.regs.lock(r);
    }

    /// A register for `out` holding `v`'s value, to be overwritten by an instruction that reads
    /// and writes it: `v`'s own where this is its last use, else a copy.
    pub fn reuse(&mut self, out: Value, v: Value) -> A::Reg {
        let src = self.reg(v);
        if let Some(r) = self.inherit(out, v) {
            return r;
        }
        let dst = self.def(out);
        A::copy(&mut self.asm, dst, src);
        dst
    }

    /// Gives `out` the register holding `v`, when this is `v`'s last use and one does: the
    /// result then overwrites the operand in place.
    pub fn inherit(&mut self, out: Value, v: Value) -> Option<A::Reg> {
        let r = self.values[v].reg?;
        if !self.dies(v) || self.values[out].fixed.is_some() {
            return None;
        }
        self.unbind(r);
        self.def_at(out, r);
        Some(r)
    }

    /// A register for the instruction compiled to use apart from any value.
    pub fn scratch(&mut self) -> A::Reg {
        let r = self.alloc();
        self.regs.lock(r);
        r
    }

    /// Frees `r` for an instruction that writes it: what it holds, is still used and is not
    /// home already moves to a free register, or is stored home when none is.
    pub fn take(&mut self, r: A::Reg) {
        if let Some(v) = self.regs.holds(r) {
            let a = self.values[v];
            let moved = a.left > 0 && !a.saved && a.fixed.is_none() && a.recompute.is_none();
            if let Some(to) = self.regs.free().filter(|_| moved) {
                A::copy(&mut self.asm, to, r);
                self.unbind(r);
                self.bind(v, to, a.saved);
            } else {
                self.store_home(v, r);
                self.unbind(r);
            }
        }
        self.regs.lock(r);
    }

    fn alloc(&mut self) -> A::Reg {
        if let Some(r) = self.regs.free() {
            return r;
        }
        let r = self.regs.victim();
        if let Some(v) = self.regs.holds(r) {
            self.store_home(v, r);
            self.unbind(r);
        }
        r
    }

    pub(super) fn bind(&mut self, v: Value, r: A::Reg, saved: bool) {
        // A value with a register of its own is moved there.
        if let Some(fixed) = self.values[v].fixed
            && fixed != r
        {
            A::copy(&mut self.asm, fixed, r);
            return self.bind(v, fixed, saved);
        }
        self.regs.set(r, Some(v));
        self.values[v].reg = Some(r);
        self.values[v].saved = saved;
    }

    pub(super) fn unbind(&mut self, r: A::Reg) {
        if let Some(v) = self.regs.holds(r) {
            self.values[v].reg = None;
            self.regs.set(r, None);
        }
    }

    /// Stores `v`, held in `r`, home when it is used later and its home does not hold it.
    pub(super) fn store_home(&mut self, v: Value, r: A::Reg) {
        let a = self.values[v];
        if a.recompute.is_none() && !a.saved && a.left > 0 && a.fixed.is_none() {
            let home = self.home(v);
            A::store(&mut self.asm, home, r);
            self.values[v].saved = true;
        }
    }

    /// After an instruction: one use of each operand is done, a value used no more frees its
    /// register, and nothing is locked.
    pub(crate) fn release<I: Iterator<Item = Value>>(
        &mut self,
        operands: impl Fn() -> I,
        result: Option<Value>,
    ) {
        for v in operands() {
            self.values[v].left = self.values[v].left.saturating_sub(1);
        }
        // Freed only once every operand is counted: one read twice is still held after the first.
        for v in operands().chain(result) {
            if self.values[v].left == 0
                && let Some(r) = self.values[v].reg
            {
                self.unbind(r);
            }
        }
        self.regs.unlock_all();
    }

    /// Counts one use of `v` as compiled outside an instruction, by a terminator; its register
    /// stays bound until the block is left (`Cx::left`), since the terminator reads it last.
    pub fn used(&mut self, v: Value) {
        self.values[v].left = self.values[v].left.saturating_sub(1);
    }

    /// Frees the registers of values used no more.
    pub(super) fn free_dead(&mut self) {
        for &r in A::ALLOCATABLE.iter().chain(A::CALLEE_SAVED) {
            if let Some(v) = self.regs.holds(r)
                && self.values[v].left == 0
            {
                self.unbind(r);
            }
        }
    }

    /// Stores home every value held in a register and still used whose home does not hold it.
    pub fn spill_live(&mut self) {
        for &r in A::ALLOCATABLE.iter().chain(A::CALLEE_SAVED) {
            if let Some(v) = self.regs.holds(r) {
                self.store_home(v, r);
            }
        }
    }

    /// Forgets what every register holds, but the values held in registers of their own.
    pub fn clear(&mut self) {
        for &r in A::ALLOCATABLE.iter().chain(A::CALLEE_SAVED) {
            if !self.is_fixed(r) {
                self.unbind(r);
            }
        }
        self.regs.unlock_all();
    }

    /// Unlocks every register, as between instructions.
    pub fn unlock(&mut self) {
        self.regs.unlock_all();
    }
}
