//! What each operation declares once (spec 13, section 8.1): how it reads its operands' bits, and
//! the operands its result is defined on. Lowering guarantees the domain, and backends rely on
//! it, so no backend emits a check the language does not have.

use crate::inst::{BinOp, CastOp, Cond};
use vun_codec::{Decode, Encode};

/// How an operation reads its operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Reads {
    /// As bits: the result is the same read either way.
    Bits,
    Unsigned,
    Signed,
    Float,
}

/// The operands an operation's result is defined on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Domain {
    Any,
    /// A shift amount below the width.
    ShiftBelowWidth,
    /// A divisor that is not 0.
    Divisor,
    /// A divisor that is not 0, and not -1 when the dividend is the minimum.
    SignedDivisor,
    /// A float whose integer part fits the result.
    Fits,
}

impl Cond {
    /// The comparison reading its operands as unsigned, for an integer one.
    #[must_use]
    pub const fn unsigned(self) -> Self {
        match self {
            Self::SLt => Self::ULt,
            Self::SLe => Self::ULe,
            Self::SGt => Self::UGt,
            Self::SGe => Self::UGe,
            other => other,
        }
    }
}

macro_rules! facts {
    ($($ty:ident { $($variant:ident => ($reads:ident, $domain:ident)),* $(,)? })*) => {$(
        impl $ty {
            #[must_use]
            pub const fn reads(self) -> Reads {
                match self { $(Self::$variant => Reads::$reads),* }
            }

            #[must_use]
            pub const fn domain(self) -> Domain {
                match self { $(Self::$variant => Domain::$domain),* }
            }
        }
    )*};
}

facts! {
    BinOp {
        Add => (Bits, Any),
        Sub => (Bits, Any),
        Mul => (Bits, Any),
        SDiv => (Signed, SignedDivisor),
        UDiv => (Unsigned, Divisor),
        SRem => (Signed, SignedDivisor),
        URem => (Unsigned, Divisor),
        And => (Bits, Any),
        Or => (Bits, Any),
        Xor => (Bits, Any),
        Shl => (Bits, ShiftBelowWidth),
        LShr => (Unsigned, ShiftBelowWidth),
        AShr => (Signed, ShiftBelowWidth),
        FAdd => (Float, Any),
        FSub => (Float, Any),
        FMul => (Float, Any),
        FDiv => (Float, Any),
    }
    Cond {
        Eq => (Bits, Any),
        Ne => (Bits, Any),
        SLt => (Signed, Any),
        SLe => (Signed, Any),
        SGt => (Signed, Any),
        SGe => (Signed, Any),
        ULt => (Unsigned, Any),
        ULe => (Unsigned, Any),
        UGt => (Unsigned, Any),
        UGe => (Unsigned, Any),
        FEq => (Float, Any),
        FNe => (Float, Any),
        FLt => (Float, Any),
        FLe => (Float, Any),
        FGt => (Float, Any),
        FGe => (Float, Any),
    }
    CastOp {
        ZExt => (Unsigned, Any),
        SExt => (Signed, Any),
        Trunc => (Bits, Any),
        FConvert => (Float, Any),
        SToF => (Signed, Any),
        UToF => (Unsigned, Any),
        FToS => (Float, Fits),
        FToU => (Float, Fits),
        PtrToInt => (Bits, Any),
        IntToPtr => (Bits, Any),
        Bitcast => (Bits, Any),
    }
}
