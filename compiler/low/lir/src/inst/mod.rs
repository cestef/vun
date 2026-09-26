//! Instructions over scalar values, and the terminators ending blocks.

mod operands;

use crate::entity::{Block, DataId, FuncId, ImportId, Slot, Value};
use crate::func::Signature;
use crate::ty::Scalar;
use vun_codec::{Decode, Encode};

/// What an instruction computes. Each defines at most one value, of the type its kind gives.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

impl BinOp {
    /// The right operand leaving the left unchanged, for an integer operation of type `ty` that
    /// has one. Floats have none: `-0.0 + 0.0` is `0.0`.
    #[must_use]
    pub fn identity(self, ty: Scalar) -> Option<u128> {
        match self {
            Self::Add | Self::Sub | Self::Or | Self::Xor | Self::Shl | Self::LShr | Self::AShr => Some(0),
            Self::Mul | Self::SDiv | Self::UDiv => Some(1),
            Self::And => ty.is_int().then(|| ty.truncate(-1)),
            Self::SRem | Self::URem | Self::FAdd | Self::FSub | Self::FMul | Self::FDiv => None,
        }
    }

    /// The right operand making the result itself whatever the left is, as 0 does of `x * 0`.
    #[must_use]
    pub fn absorbing(self, ty: Scalar) -> Option<u128> {
        match self {
            Self::Mul | Self::And => ty.is_int().then_some(0),
            Self::Or => ty.is_int().then(|| ty.truncate(-1)),
            _ => None,
        }
    }

    /// What `a op b` is where only `b`, of type `ty`, is known.
    #[must_use]
    pub fn with_right(self, ty: Scalar, b: u128) -> Option<Right> {
        if self.identity(ty) == Some(b) {
            Some(Right::Left)
        } else {
            (self.absorbing(ty) == Some(b)).then_some(Right::Constant(b))
        }
    }
}

/// What an operation gives where its right operand alone is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    /// Its left operand, as `x * 1` does.
    Left,
    /// This constant, as `x * 0` does.
    Constant(u128),
}

/// A comparison: integers as signed or unsigned, floats ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Access {
    Plain,
    /// Performed exactly as written: memory-mapped registers (`lang.mem.volatile_*`).
    Volatile,
    Atomic(Order),
}

/// A memory ordering of an atomic access (`lang.mem.Ordering`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Order {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum RmwOp {
    Swap,
    Add,
    Sub,
    And,
    Or,
    Xor,
}

/// Something with an address in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Address {
    Func(FuncId),
    Data(DataId),
    Import(ImportId),
}

/// What a call calls.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Callee {
    Func(FuncId),
    Import(ImportId),
    /// A function at the address in the value, of the signature given.
    Indirect(Value, Box<Signature>),
}

/// A jump to a block, with the values of its parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Target {
    pub block: Block,
    pub args: Vec<Value>,
}

/// How a block ends.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

impl Address {
    /// What a call of this address calls; data is not called.
    #[must_use]
    pub fn callee(self) -> Option<Callee> {
        match self {
            Self::Func(f) => Some(Callee::Func(f)),
            Self::Import(i) => Some(Callee::Import(i)),
            Self::Data(_) => None,
        }
    }
}

impl InstKind {
    /// Whether the instruction does more than define its value: it writes memory, calls, or
    /// reads memory as written. One that does not, and whose value nothing reads, can go.
    #[must_use]
    pub fn acts(&self) -> bool {
        match self {
            Self::Store { .. }
            | Self::Rmw { .. }
            | Self::CompareExchange { .. }
            | Self::Copy { .. }
            | Self::Call { .. } => true,
            Self::Load { access, .. } => *access != Access::Plain,
            Self::Const { .. }
            | Self::Binary { .. }
            | Self::Compare { .. }
            | Self::Cast { .. }
            | Self::Select { .. }
            | Self::Offset { .. }
            | Self::SlotAddr(_)
            | Self::Addr(_) => false,
        }
    }
}
