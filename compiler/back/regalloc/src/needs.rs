//! What each point of a function needs of the places of the values it reads and writes, as the
//! architecture describes it.

use vun_lir::{Block, Function, Inst, Scalar, Value};

use crate::machine::{Bank, Machine, PReg, RegSet};

/// Where an operand must be when it is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// In a register of its bank.
    Reg,
    /// In this register.
    Fixed(PReg),
    /// In a register or in its home, as an instruction taking a memory operand reads it.
    Any,
}

/// Where a result must be when it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Reg,
    Fixed(PReg),
    /// In the register of the operand at this index, which the instruction overwrites: the
    /// other operands are read until the result is written, so none of them shares it.
    Reuse(usize),
    /// In its home: a parameter the caller passes in memory.
    Home,
    /// Wherever the allocator puts it: a block parameter, which the jumps to it fill.
    Any,
}

/// An operand a point reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    pub value: Value,
    pub need: Need,
    /// Whether it is read again after the result is written, as a snippet may read an operand
    /// after writing its result: no result then shares its register.
    pub late: bool,
}

/// What one point needs: an instruction, a terminator, or a block's entry defining its
/// parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub uses: Vec<Read>,
    pub defs: Vec<(Value, Out)>,
    /// Registers it writes once its operands are read, such as those a call does not preserve;
    /// its own results may be held there.
    pub clobbers: RegSet,
    /// Registers it takes for itself while it runs: none of its operands is held there, nor a
    /// result but one fixed there.
    pub reserved: RegSet,
    /// Scratch registers it takes, of each bank, apart from any value.
    pub temps: Vec<Bank>,
}

impl Needs {
    /// Reads `value`, where `need` says, before the result is written.
    pub fn read(&mut self, value: Value, need: Need) {
        self.uses.push(Read { value, need, late: false });
    }

    pub fn clear(&mut self) {
        self.uses.clear();
        self.defs.clear();
        self.clobbers = RegSet::EMPTY;
        self.reserved = RegSet::EMPTY;
        self.temps.clear();
    }
}

/// What an architecture tells the allocator about a function.
pub trait Describe {
    fn machine(&self) -> &Machine;

    /// The bank values of type `ty` are held in.
    fn bank(&self, ty: Scalar) -> Bank;

    /// Whether `v` is recomputed where used rather than kept anywhere: a constant or an address
    /// known while compiling.
    fn recomputed(&self, v: Value) -> bool;

    /// What `inst` needs.
    fn inst(&self, f: &Function, inst: Inst, needs: &mut Needs);

    /// What the terminator of `block` needs of the values it reads itself (a condition, a value
    /// returned), not of the arguments its jumps pass.
    fn term(&self, f: &Function, block: Block, needs: &mut Needs);

    /// Where the entry receives the function's parameters.
    fn entry(&self, f: &Function, needs: &mut Needs);
}
