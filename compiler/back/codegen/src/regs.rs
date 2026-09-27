//! The register file at the current point: what each register holds, which are locked for the
//! instruction compiled, and the next one evicted when none is free.

use std::marker::PhantomData;

use vun_lir::Value;

use crate::Arch;

pub(crate) struct RegFile<A: Arch> {
    holds: Vec<Option<Value>>,
    /// Registers an instruction reads or writes, by index: nothing evicts them.
    locked: u64,
    victim: usize,
    arch: PhantomData<A>,
}

impl<A: Arch> RegFile<A> {
    pub(crate) fn new() -> Self {
        const { assert!(A::REGS <= 64, "locks are a 64-bit set") };
        Self { holds: vec![None; A::REGS], locked: 0, victim: 0, arch: PhantomData }
    }

    pub(crate) fn holds(&self, r: A::Reg) -> Option<Value> {
        self.holds[A::index(r)]
    }

    pub(crate) fn set(&mut self, r: A::Reg, v: Option<Value>) {
        self.holds[A::index(r)] = v;
    }

    pub(crate) fn lock(&mut self, r: A::Reg) {
        self.locked |= 1 << A::index(r);
    }

    pub(crate) fn is_locked(&self, r: A::Reg) -> bool {
        self.locked & 1 << A::index(r) != 0
    }

    pub(crate) fn unlock_all(&mut self) {
        self.locked = 0;
    }

    /// The lowest allocatable register neither holding a value nor locked.
    pub(crate) fn free(&self) -> Option<A::Reg> {
        A::ALLOCATABLE.iter().copied().find(|&r| self.holds(r).is_none() && !self.is_locked(r))
    }

    /// The next register to evict, round-robin, never a locked one.
    ///
    /// # Panics
    /// Panics if every allocatable register is locked: an instruction reads more values than
    /// the architecture has registers.
    pub(crate) fn victim(&mut self) -> A::Reg {
        for _ in 0..A::ALLOCATABLE.len() {
            let r = A::ALLOCATABLE[self.victim % A::ALLOCATABLE.len()];
            self.victim += 1;
            if !self.is_locked(r) {
                return r;
            }
        }
        panic!("every register is locked by one instruction")
    }
}
