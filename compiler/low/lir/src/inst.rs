//! Instructions over scalar values, and the terminators ending blocks.

use crate::entity::{Block, DataId, FuncId, ImportId, Slot, Value};
use crate::func::Signature;
use crate::ty::Scalar;

/// What an instruction computes. Each defines at most one value, of the type its kind gives.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InstKind {
    /// A constant of a scalar type, as its bits: a float's IEEE encoding, an integer's two's
    /// complement truncated to the type.
    Const { ty: Scalar, bits: u128 },
    /// Two operands of one type, and a result of that type.
    Binary { op: BinOp, a: Value, b: Value },
    /// Two operands of one type; the result is `i8`, 1 when the comparison holds.
    Compare { cond: Cond, a: Value, b: Value },
    /// `value` converted to `to`.
    Cast { op: CastOp, value: Value, to: Scalar },
    /// `then` where `cond` is not 0, `otherwise` where it is.
    Select { cond: Value, then: Value, otherwise: Value },
    /// The scalar of type `ty` at `addr`.
    Load { ty: Scalar, addr: Value, access: Access },
    /// Writes `value` at `addr`.
    Store { value: Value, addr: Value, access: Access },
    /// `op` applied to the scalar at `addr` and `value` atomically, giving the scalar there before.
    Rmw { op: RmwOp, addr: Value, value: Value, order: Order },
    /// Writes `new` at `addr` when it holds `expected`, atomically; gives what it held before.
    CompareExchange { addr: Value, expected: Value, new: Value, success: Order, failure: Order },
    /// The address `offset` bytes, an integer of the address's width, past `base`.
    Offset { base: Value, offset: Value },
    /// The address of a stack slot.
    SlotAddr(Slot),
    /// The address of a function, data object or linked symbol.
    Addr(Address),
    /// Copies `bytes` bytes from `from` to `to`, which do not overlap.
    Copy { to: Value, from: Value, bytes: u64 },
    /// A call; its result, if the signature has one, is the instruction's value.
    Call { callee: Callee, args: Vec<Value> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// Division rounding toward zero, of signed or unsigned operands.
    SDiv,
    UDiv,
    /// The remainder of the division of the same name.
    SRem,
    URem,
    And,
    Or,
    Xor,
    /// Shifts by an amount below the width.
    Shl,
    LShr,
    AShr,
    FAdd,
    FSub,
    FMul,
    FDiv,
}

/// A comparison: integers as signed or unsigned, floats ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cond {
    Eq,
    Ne,
    SLt,
    SLe,
    SGt,
    SGe,
    ULt,
    ULe,
    UGt,
    UGe,
    FEq,
    FNe,
    FLt,
    FLe,
    FGt,
    FGe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CastOp {
    /// Widens an integer, with zeros or copies of its sign bit.
    ZExt,
    SExt,
    /// Keeps the low bits of an integer.
    Trunc,
    /// Between floats of different widths.
    FConvert,
    /// Between floats and signed or unsigned integers.
    SToF,
    UToF,
    FToS,
    FToU,
    /// An address as an integer of its width, and back.
    PtrToInt,
    IntToPtr,
    /// The same bits as another scalar of the same width.
    Bitcast,
}

/// How a load or store touches memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Access {
    Plain,
    /// Performed exactly as written: memory-mapped registers (`lang.mem.volatile_*`).
    Volatile,
    Atomic(Order),
}

/// A memory ordering of an atomic access (`lang.mem.Ordering`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Order {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RmwOp {
    Swap,
    Add,
    Sub,
    And,
    Or,
    Xor,
}

/// Something with an address in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Address {
    Func(FuncId),
    Data(DataId),
    Import(ImportId),
}

/// What a call calls.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Callee {
    Func(FuncId),
    Import(ImportId),
    /// A function at the address in the value, of the signature given.
    Indirect(Value, Box<Signature>),
}

/// A jump to a block, with the values of its parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    pub block: Block,
    pub args: Vec<Value>,
}

/// How a block ends.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Terminator {
    Jump(Target),
    /// To `then` where `cond` is not 0, to `otherwise` where it is.
    Branch {
        cond: Value,
        then: Target,
        otherwise: Target,
    },
    /// To the target of the case equal to the integer `value`, or to `default`.
    Switch {
        value: Value,
        cases: Vec<(u128, Target)>,
        default: Target,
    },
    /// Returns the function's result, if its signature has one.
    Return(Option<Value>),
    /// Control never gets here.
    Unreachable,
}

impl Terminator {
    /// The jumps this may take.
    #[must_use]
    pub fn targets(&self) -> Vec<&Target> {
        match self {
            Self::Jump(t) => vec![t],
            Self::Branch { then, otherwise, .. } => vec![then, otherwise],
            Self::Switch { cases, default, .. } => cases.iter().map(|(_, t)| t).chain([default]).collect(),
            Self::Return(_) | Self::Unreachable => Vec::new(),
        }
    }

    /// The values this reads, jump arguments included.
    #[must_use]
    pub fn operands(&self) -> Vec<Value> {
        let own = match self {
            Self::Branch { cond, .. } => vec![*cond],
            Self::Switch { value, .. } => vec![*value],
            Self::Return(value) => value.iter().copied().collect(),
            Self::Jump(_) | Self::Unreachable => Vec::new(),
        };
        own.into_iter().chain(self.targets().into_iter().flat_map(|t| t.args.iter().copied())).collect()
    }
}

impl InstKind {
    /// The values this reads.
    #[must_use]
    pub fn operands(&self) -> Vec<Value> {
        match self {
            Self::Const { .. } | Self::SlotAddr(_) | Self::Addr(_) => Vec::new(),
            Self::Binary { a, b, .. } | Self::Compare { a, b, .. } => vec![*a, *b],
            Self::Cast { value, .. } => vec![*value],
            Self::Select { cond, then, otherwise } => vec![*cond, *then, *otherwise],
            Self::Load { addr, .. } => vec![*addr],
            Self::Store { value, addr, .. } | Self::Rmw { addr, value, .. } => vec![*value, *addr],
            Self::CompareExchange { addr, expected, new, .. } => vec![*addr, *expected, *new],
            Self::Offset { base, offset } => vec![*base, *offset],
            Self::Copy { to, from, .. } => vec![*to, *from],
            Self::Call { callee: Callee::Indirect(f, _), args } => {
                std::iter::once(*f).chain(args.iter().copied()).collect()
            }
            Self::Call { args, .. } => args.clone(),
        }
    }
}
