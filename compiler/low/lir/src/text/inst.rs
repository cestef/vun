//! Instructions and terminators as text: `v3 = add v1, v2`, `br v4, b1(v2), b2`.

use std::fmt;

use vun_index::Idx;

use crate::entity::{Block, DataId, FuncId, ImportId, Slot, Value};
use crate::inst::{Access, Address, BinOp, Callee, CastOp, Cond, InstKind, Order, RmwOp, Target, Terminator};

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.index())
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "b{}", self.index())
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.index())
    }
}

impl fmt::Display for FuncId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "f{}", self.index())
    }
}

impl fmt::Display for DataId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "d{}", self.index())
    }
}

impl fmt::Display for ImportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "i{}", self.index())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Func(id) => id.fmt(f),
            Self::Data(id) => id.fmt(f),
            Self::Import(id) => id.fmt(f),
        }
    }
}

/// Values separated by commas.
struct List<'v>(&'v [Value]);

impl fmt::Display for List<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            write!(f, "{}{v}", if i > 0 { ", " } else { "" })?;
        }
        Ok(())
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.block)?;
        if !self.args.is_empty() {
            write!(f, "({})", List(&self.args))?;
        }
        Ok(())
    }
}

macro_rules! names {
    ($($ty:ident { $($variant:ident => $name:literal),* $(,)? })*) => {$(
        impl $ty {
            /// The name the text format writes.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),* }
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.name())
            }
        }
    )*};
}

names! {
    BinOp {
        Add => "add", Sub => "sub", Mul => "mul", SDiv => "sdiv", UDiv => "udiv", SRem => "srem",
        URem => "urem", And => "and", Or => "or", Xor => "xor", Shl => "shl", LShr => "lshr",
        AShr => "ashr", FAdd => "fadd", FSub => "fsub", FMul => "fmul", FDiv => "fdiv",
    }
    Cond {
        Eq => "eq", Ne => "ne", SLt => "slt", SLe => "sle", SGt => "sgt", SGe => "sge", ULt => "ult",
        ULe => "ule", UGt => "ugt", UGe => "uge", FEq => "feq", FNe => "fne", FLt => "flt",
        FLe => "fle", FGt => "fgt", FGe => "fge",
    }
    CastOp {
        ZExt => "zext", SExt => "sext", Trunc => "trunc", FConvert => "fconvert", SToF => "stof",
        UToF => "utof", FToS => "ftos", FToU => "ftou", PtrToInt => "ptrtoint",
        IntToPtr => "inttoptr", Bitcast => "bitcast",
    }
    Order { Relaxed => "relaxed", Acquire => "acquire", Release => "release", AcqRel => "acqrel", SeqCst => "seqcst" }
    RmwOp { Swap => "swap", Add => "add", Sub => "sub", And => "and", Or => "or", Xor => "xor" }
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain => Ok(()),
            Self::Volatile => f.write_str(" volatile"),
            Self::Atomic(order) => write!(f, " atomic {order}"),
        }
    }
}

impl fmt::Display for InstKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Const { ty, bits } => write!(f, "const {ty} {bits}"),
            Self::Binary { op, a, b } => write!(f, "{op} {a}, {b}"),
            Self::Compare { cond, a, b } => write!(f, "cmp {cond} {a}, {b}"),
            Self::Cast { op, value, to } => write!(f, "{op} {value} to {to}"),
            Self::Select { cond, then, otherwise } => write!(f, "select {cond}, {then}, {otherwise}"),
            Self::Load { ty, addr, access } => write!(f, "load{access} {ty} {addr}"),
            Self::Store { value, addr, access } => write!(f, "store{access} {value}, {addr}"),
            Self::Rmw { op, addr, value, order } => write!(f, "rmw {op} {order} {addr}, {value}"),
            Self::CompareExchange { addr, expected, new, success, failure } => {
                write!(f, "cmpxchg {success} {failure} {addr}, {expected}, {new}")
            }
            Self::Offset { base, offset } => write!(f, "offset {base}, {offset}"),
            Self::SlotAddr(slot) => write!(f, "addr {slot}"),
            Self::Addr(address) => write!(f, "addr {address}"),
            Self::Copy { to, from, bytes } => write!(f, "copy {to}, {from}, {bytes}"),
            Self::Call { callee, args } => {
                match callee {
                    Callee::Func(id) => write!(f, "call {id}")?,
                    Callee::Import(id) => write!(f, "call {id}")?,
                    Callee::Indirect(value, _) => write!(f, "call_indirect {value}")?,
                }
                write!(f, "({})", List(args))
            }
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jump(target) => write!(f, "jump {target}"),
            Self::Branch { cond, then, otherwise } => write!(f, "br {cond}, {then}, {otherwise}"),
            Self::Switch { value, cases, default } => {
                write!(f, "switch {value}")?;
                for (case, target) in cases {
                    write!(f, ", {case} => {target}")?;
                }
                write!(f, ", _ => {default}")
            }
            Self::Return(Some(value)) => write!(f, "ret {value}"),
            Self::Return(None) => f.write_str("ret"),
            Self::Unreachable => f.write_str("unreachable"),
        }
    }
}
