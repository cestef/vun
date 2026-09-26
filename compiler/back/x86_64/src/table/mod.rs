//! The instructions the backend emits, each declared once with its forms: the operand kinds a
//! form takes, the widths it is defined at, and how it is encoded. The encoder, the printer and
//! form selection (TPDE's encoding candidates: the first form whose operands fit) all read this
//! table, so an instruction's encoding and its text cannot disagree.

mod form;
mod int;
mod sse;

pub use form::{Form, Kind, Layout, Prefix, W};

use int::{
    CALL, CMOV, CMPXCHG, CQO, IMUL, JCC, JMP, LEA, LEAVE, MOV, MOVSB, MOVSX, MOVZX, POP, PUSH, RET, SET,
    TEST, UD2, WIDE_ACCUMULATOR, XADD, XCHG, alu, shift, unary,
};
use sse::{CVTSD2SS, CVTSI2SD, CVTSI2SS, CVTSS2SD, CVTTSD2SI, CVTTSS2SI, MOVD, scalar};

macro_rules! ops {
    ($($(#[$doc:meta])* $op:ident $mnemonic:literal = $forms:expr;)*) => {
        /// An instruction, by its mnemonic; one taking a condition code names the family
        /// (`J` for `jcc`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op {
            $($(#[$doc])* $op,)*
        }

        impl Op {
            pub const ALL: &[Self] = &[$(Self::$op),*];

            /// The mnemonic, or its stem for an instruction taking a condition code.
            #[must_use]
            pub const fn mnemonic(self) -> &'static str {
                match self {
                    $(Self::$op => $mnemonic,)*
                }
            }

            /// Its forms, in the order they are tried: shorter encodings first.
            #[must_use]
            pub const fn forms(self) -> &'static [Form] {
                match self {
                    $(Self::$op => {
                        const FORMS: &[Form] = $forms;
                        FORMS
                    })*
                }
            }
        }
    };
}

ops! {
    Mov "mov" = &MOV;
    /// Zero-extends an operand of the instruction's size to 32 bits, or 64.
    Movzx "movzx" = &MOVZX;
    /// Sign-extends an operand of the instruction's size to 64 bits.
    Movsx "movsx" = &MOVSX;
    Lea "lea" = &LEA;
    Add "add" = &alu(0);
    Or "or" = &alu(1);
    And "and" = &alu(4);
    Sub "sub" = &alu(5);
    Xor "xor" = &alu(6);
    Cmp "cmp" = &alu(7);
    Test "test" = &TEST;
    Imul "imul" = &IMUL;
    Not "not" = &unary(2, &[]);
    Neg "neg" = &unary(3, &[]);
    Mul "mul" = &unary(4, WIDE_ACCUMULATOR);
    Div "div" = &unary(6, WIDE_ACCUMULATOR);
    Idiv "idiv" = &unary(7, WIDE_ACCUMULATOR);
    Shl "shl" = &shift(4);
    Shr "shr" = &shift(5);
    Sar "sar" = &shift(7);
    /// Sign-extends `rax` into `rdx`, `eax` into `edx` at 32 bits (`cdq`).
    Cqo "cqo" = &CQO;
    Set "set" = &SET;
    Cmov "cmov" = &CMOV;
    J "j" = &JCC;
    Jmp "jmp" = &JMP;
    Call "call" = &CALL;
    Ret "ret" = &RET;
    Push "push" = &PUSH;
    Pop "pop" = &POP;
    Leave "leave" = &LEAVE;
    Ud2 "ud2" = &UD2;
    Xchg "xchg" = &XCHG;
    LockXadd "lock xadd" = &XADD;
    LockCmpxchg "lock cmpxchg" = &CMPXCHG;
    /// Copies `rcx` bytes from `[rsi]` to `[rdi]`.
    RepMovsb "rep movsb" = &MOVSB;
    /// `movd` at 32 bits, `movq` at 64.
    Movd "movd" = &MOVD;
    /// `addss` at 32 bits, `addsd` at 64; so for each scalar operation.
    Adds "adds" = &scalar(0x58, (Prefix::F3, "addss"), (Prefix::F2, "addsd"));
    Subs "subs" = &scalar(0x5c, (Prefix::F3, "subss"), (Prefix::F2, "subsd"));
    Muls "muls" = &scalar(0x59, (Prefix::F3, "mulss"), (Prefix::F2, "mulsd"));
    Divs "divs" = &scalar(0x5e, (Prefix::F3, "divss"), (Prefix::F2, "divsd"));
    /// Sets the flags from comparing floats, unordered where either is a NaN.
    Ucomis "ucomis" = &scalar(0x2e, (Prefix::None, "ucomiss"), (Prefix::P66, "ucomisd"));
    Cvtss2sd "cvtss2sd" = &CVTSS2SD;
    Cvtsd2ss "cvtsd2ss" = &CVTSD2SS;
    /// A signed integer of the instruction's size, rounded to single precision.
    Cvtsi2ss "cvtsi2ss" = &CVTSI2SS;
    Cvtsi2sd "cvtsi2sd" = &CVTSI2SD;
    /// Single precision truncated toward zero, a signed integer of the instruction's size.
    Cvttss2si "cvttss2si" = &CVTTSS2SI;
    Cvttsd2si "cvttsd2si" = &CVTTSD2SI;
}

use crate::operand::Operand;
use crate::reg::{Reg, Size};

impl Op {
    /// The registers it reads or writes without naming them, whichever form encodes it.
    #[must_use]
    pub const fn implicit(self) -> &'static [Reg] {
        self.forms()[0].implicit
    }

    /// The first form taking `operands` at `size`.
    #[must_use]
    pub fn form<S>(self, size: Size, operands: &[Operand<S>]) -> Option<&'static Form> {
        self.forms().iter().find(|f| f.takes(size, operands))
    }
}
