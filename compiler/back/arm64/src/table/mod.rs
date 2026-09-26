//! The instructions the backend emits, each declared once with its forms: the base word, the
//! widths a form is defined at and the bits each sets, and the operand kinds with where their
//! fields go. The encoder, the printer and form selection (TPDE's encoding candidates: the first
//! form whose operands fit) all read this table, so an instruction's encoding and its text cannot
//! disagree.

mod control;
mod form;
mod fp;
mod int;
mod kind;
mod mem;
mod shared;

pub use form::{Arg, Form, Sized};
pub(crate) use kind::{ARITH_BITS, HALF_BITS};
pub use kind::{Branch, Cond, Imm, Kind, Mode, R31, Shifts, Width};

use control::{ADRP, B, BCOND, BL, BLR, BR, BRK, CBNZ, CBZ, DMB_ISH, RET, UDF};
use form::{A, D, N};
use fp::{
    FABS, FADD, FCMP, FCVT, FCVTZS, FCVTZU, FDIV, FMOV, FMUL, FNEG, FSQRT, FSUB, SCVTF, UCVTF, binary,
    from_int, to_int, unary as fp_unary,
};
use int::{
    ADD_FORMS, ADDS, AND, ANDS, CSINC, CSINV, CSNEG, EOR, MADD, MNEG, MOV, MOVK, MSUB, MUL, MVN, ORR, SBFM,
    SDIV, SHL, SHR, SMULH, SUB, SUBS, SXTW, TST, UBFM, UDIV, UMULH, arith, compare, logic, negate, negated,
    select, set, shift, sign, unary, zero,
};
use mem::{
    ACQUIRE, CAS, CAS_ACQUIRE, CAS_RELEASE, FP, LDADD, LDAR, LDAXR, LDXR, LOAD, LOAD_SIGNED, PAIR_LOAD,
    RELEASE, STLR, STLXR, STORE, STXR, SWP, access, atomic, discarding, exclusive, ordered, pair,
};
use shared::{ACCESS, ARITH, BITMASK, F, FP_ACCESS, FP_PAIRS, PAIRS, R, RSP, SIGNED, X};

use crate::operand::Operand;
use crate::reg::Size;

macro_rules! ops {
    ($($(#[$doc:meta])* $op:ident $mnemonic:literal = $forms:expr;)*) => {
        /// An instruction, by its mnemonic.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op {
            $($(#[$doc])* $op,)*
        }

        impl Op {
            pub const ALL: &[Self] = &[$(Self::$op),*];

            /// The mnemonic its forms are written with, unless a form names its own.
            #[must_use]
            pub const fn mnemonic(self) -> &'static str {
                match self {
                    $(Self::$op => $mnemonic,)*
                }
            }

            /// Its forms, in the order they are tried.
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
    /// Moves 16 bits into a register, keeping the others.
    Movk "movk" = &MOVK;
    Add "add" = &ADD_FORMS;
    Adds "adds" = &arith(ADDS, &[R.at(D), RSP.at(N), ARITH.at(A)], &[R.at(D), RSP.at(N), Kind::Extended.at(A)]);
    Sub "sub" = &arith(SUB, &[RSP.at(D), RSP.at(N), ARITH.at(A)], &[RSP.at(D), RSP.at(N), Kind::Extended.at(A)]);
    Subs "subs" = &arith(SUBS, &[R.at(D), RSP.at(N), ARITH.at(A)], &[R.at(D), RSP.at(N), Kind::Extended.at(A)]);
    Cmp "cmp" = &compare(SUBS);
    Cmn "cmn" = &compare(ADDS);
    Neg "neg" = &negate(SUB);
    Negs "negs" = &negate(SUBS);
    Mul "mul" = &MUL;
    Madd "madd" = &MADD;
    Msub "msub" = &MSUB;
    Mneg "mneg" = &MNEG;
    /// The high 64 bits of a signed product.
    Smulh "smulh" = &SMULH;
    Umulh "umulh" = &UMULH;
    Sdiv "sdiv" = &SDIV;
    Udiv "udiv" = &UDIV;
    And "and" = &logic(AND, &[RSP.at(D), R.at(N), BITMASK.at(A)]);
    Ands "ands" = &logic(ANDS, &[R.at(D), R.at(N), BITMASK.at(A)]);
    Orr "orr" = &logic(ORR, &[RSP.at(D), R.at(N), BITMASK.at(A)]);
    Eor "eor" = &logic(EOR, &[RSP.at(D), R.at(N), BITMASK.at(A)]);
    Bic "bic" = &negated(AND);
    Orn "orn" = &negated(ORR);
    Tst "tst" = &TST;
    Mvn "mvn" = &MVN;
    Lsl "lsl" = &shift(0, UBFM, SHL);
    Lsr "lsr" = &shift(0x400, UBFM, SHR);
    Asr "asr" = &shift(0x800, SBFM, SHR);
    /// Sign-extends the low byte of a 32-bit register to the instruction's width.
    Sxtb "sxtb" = &sign(7);
    Sxth "sxth" = &sign(15);
    Sxtw "sxtw" = &SXTW;
    /// Zero-extends the low byte to 32 bits, which clears the upper half too.
    Uxtb "uxtb" = &zero(7);
    Uxth "uxth" = &zero(15);
    Csel "csel" = &select(0);
    Csinc "csinc" = &select(CSINC);
    Csinv "csinv" = &select(CSINV);
    Csneg "csneg" = &select(CSNEG);
    /// 1 if the condition holds, else 0.
    Cset "cset" = &set(CSINC);
    /// All ones if the condition holds, else 0.
    Csetm "csetm" = &set(CSINV);
    Cinc "cinc" = &unary(CSINC);
    Cinv "cinv" = &unary(CSINV);
    Cneg "cneg" = &unary(CSNEG);
    B "b" = &B;
    /// A conditional branch, written with its condition as the mnemonic's suffix (`b.eq`).
    Bcond "b" = &BCOND;
    Bl "bl" = &BL;
    Br "br" = &BR;
    Blr "blr" = &BLR;
    Ret "ret" = &RET;
    Cbz "cbz" = &CBZ;
    Cbnz "cbnz" = &CBNZ;
    Udf "udf" = &UDF;
    Brk "brk" = &BRK;
    /// The page of an address, which a following `add` or load completes with its low 12 bits.
    Adrp "adrp" = &ADRP;
    /// A load of the instruction's width, zero-extended.
    Ldr "ldr" = &access!("ldur": (LOAD, R, ACCESS), (LOAD | FP, F, FP_ACCESS));
    Str "str" = &access!("stur": (STORE, R, ACCESS), (STORE | FP, F, FP_ACCESS));
    /// A load of the instruction's width, sign-extended to 64 bits.
    Ldrs "ldrs" = &access!("ldurs": (LOAD_SIGNED, X, SIGNED));
    Ldp "ldp" = &pair!((PAIR_LOAD, R, PAIRS), (PAIR_LOAD | FP, F, FP_PAIRS));
    Stp "stp" = &pair!((0, R, PAIRS), (FP, F, FP_PAIRS));
    Ldar "ldar" = &ordered(LDAR);
    Stlr "stlr" = &ordered(STLR);
    Ldaxr "ldaxr" = &ordered(LDAXR);
    Ldxr "ldxr" = &ordered(LDXR);
    Stxr "stxr" = &exclusive(STXR);
    Stlxr "stlxr" = &exclusive(STLXR);
    Ldadd "ldadd" = &atomic(LDADD);
    Ldadda "ldadda" = &atomic(LDADD | ACQUIRE);
    Ldaddl "ldaddl" = &atomic(LDADD | RELEASE);
    Ldaddal "ldaddal" = &atomic(LDADD | ACQUIRE | RELEASE);
    /// `ldadd` discarding what memory held.
    Stadd "stadd" = &discarding(LDADD);
    Staddl "staddl" = &discarding(LDADD | RELEASE);
    Swp "swp" = &atomic(SWP);
    Swpa "swpa" = &atomic(SWP | ACQUIRE);
    Swpl "swpl" = &atomic(SWP | RELEASE);
    Swpal "swpal" = &atomic(SWP | ACQUIRE | RELEASE);
    /// Compares the first register with memory and, if equal, stores the second; the first
    /// receives what memory held.
    Cas "cas" = &atomic(CAS);
    Casa "casa" = &atomic(CAS | CAS_ACQUIRE);
    Casl "casl" = &atomic(CAS | CAS_RELEASE);
    Casal "casal" = &atomic(CAS | CAS_ACQUIRE | CAS_RELEASE);
    /// A full barrier within the inner shareable domain.
    DmbIsh "dmb ish" = &DMB_ISH;
    Fmov "fmov" = &FMOV;
    Fadd "fadd" = &binary(FADD);
    Fsub "fsub" = &binary(FSUB);
    Fmul "fmul" = &binary(FMUL);
    Fdiv "fdiv" = &binary(FDIV);
    Fneg "fneg" = &fp_unary(FNEG);
    Fabs "fabs" = &fp_unary(FABS);
    Fsqrt "fsqrt" = &fp_unary(FSQRT);
    Fcmp "fcmp" = &FCMP;
    Fcvt "fcvt" = &FCVT;
    /// A signed integer of the instruction's width to single precision.
    ScvtfS "scvtf" = &from_int(SCVTF)[0];
    ScvtfD "scvtf" = &from_int(SCVTF)[1];
    UcvtfS "ucvtf" = &from_int(UCVTF)[0];
    UcvtfD "ucvtf" = &from_int(UCVTF)[1];
    /// Single precision to a signed integer of the instruction's width, rounded toward zero.
    FcvtzsS "fcvtzs" = &to_int(FCVTZS)[0];
    FcvtzsD "fcvtzs" = &to_int(FCVTZS)[1];
    FcvtzuS "fcvtzu" = &to_int(FCVTZU)[0];
    FcvtzuD "fcvtzu" = &to_int(FCVTZU)[1];
}

impl Op {
    /// The first form taking `operands` at `size`, and the word it encodes them to.
    #[must_use]
    pub fn select<S>(self, size: Size, operands: &[Operand<S>]) -> Option<(&'static Form, u32)> {
        self.forms().iter().find_map(|f| f.encode(size, operands).map(|word| (f, word)))
    }

    /// The first form taking `operands` at `size`.
    #[must_use]
    pub fn form<S>(self, size: Size, operands: &[Operand<S>]) -> Option<&'static Form> {
        self.select(size, operands).map(|(form, _)| form)
    }
}
