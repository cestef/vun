//! Operands covering each kind's cases: registers low and high, `sp` and `pc`; immediates each
//! encoding holds and some it does not; memory at every offset's bounds.

use vun_isa_thumb::{
    Barrier, Cc, Coprocessor, Cr, Kind, List, Mask, Mem, Operand, Reg, Shift, ShiftKind, Special, Status, V,
};

use super::Sym;

const REGS: [Reg; 6] = [Reg::R0, Reg::R7, Reg::R9, Reg::Sp, Reg::Lr, Reg::Pc];
const VS: [V; 4] = [V::V0, V::V1, V::V14, V::V31];
const IMMS: [i64; 21] = [
    0,
    1,
    7,
    8,
    31,
    32,
    255,
    256,
    508,
    1020,
    1024,
    0x3fc,
    0x00ab_00ab,
    0xab00_ab00,
    0xabab_abab,
    4095,
    4096,
    0xffff,
    0x1_0000,
    -1,
    -4,
];
const OFFSETS: [i64; 12] = [0, 1, 4, 124, 128, 255, 256, 1020, 4095, -4, -255, -1020];
const CCS: [Cc; 4] = [Cc::Eq, Cc::Ne, Cc::Ge, Cc::Hi];
const COPROCESSORS: [Coprocessor; 2] = [Coprocessor::P14, Coprocessor::P15];
const CRS: [Cr; 3] = [Cr::C0, Cr::C7, Cr::C13];

/// The candidates for a position of `kind`.
pub(super) fn candidates(kind: Kind) -> Vec<Operand<Sym>> {
    let regs = REGS.map(Operand::Reg).to_vec();
    match kind {
        Kind::Gpr(_) | Kind::Split => regs,
        Kind::Fpr(..) => VS.map(Operand::V).to_vec(),
        Kind::Imm(_) => IMMS.map(Operand::Imm).to_vec(),
        Kind::Shifted => {
            let shifted = [
                (Reg::R1, ShiftKind::Lsl, 3),
                (Reg::R9, ShiftKind::Lsr, 32),
                (Reg::R2, ShiftKind::Asr, 1),
                (Reg::R3, ShiftKind::Ror, 7),
            ];
            let shifted = shifted.map(|(r, kind, amount)| Operand::Shifted(r, Shift { kind, amount }));
            [&regs[..], &shifted].concat()
        }
        Kind::Mem(..) => mems(),
        Kind::Cond | Kind::Then(_) => CCS.map(Operand::Cond).to_vec(),
        Kind::List(_) => lists(),
        Kind::Special(_) => Special::ALL.map(Operand::Special).to_vec(),
        Kind::Status(_) => Status::ALL.map(Operand::Status).to_vec(),
        Kind::Mask => Mask::ALL.map(Operand::Mask).to_vec(),
        Kind::Barrier => Barrier::ALL.map(Operand::Barrier).to_vec(),
        Kind::Coprocessor => COPROCESSORS.map(Operand::Coprocessor).to_vec(),
        Kind::Cr => CRS.map(Operand::Cr).to_vec(),
        Kind::Target(_) | Kind::Low | Kind::High => Vec::new(),
    }
}

/// Memory of every mode, off low and high bases and `sp`.
fn mems() -> Vec<Operand<Sym>> {
    let mut all = Vec::new();
    for base in [Reg::R1, Reg::R9, Reg::Sp] {
        for offset in OFFSETS {
            all.extend([Mem::Off { base, offset }, Mem::Pre { base, offset }, Mem::Post { base, offset }]);
        }
    }
    all.extend([
        Mem::index(Reg::R1, Reg::R2),
        Mem::Index { base: Reg::R9, index: Reg::R10, shift: 3 },
        Mem::Index { base: Reg::R1, index: Reg::R8, shift: 2 },
    ]);
    all.into_iter().map(Operand::Mem).collect()
}

/// Lists of registers, low and high, with `lr` or `pc`, and of double-precision registers.
fn lists() -> Vec<Operand<Sym>> {
    let regs = |rs: &[Reg]| List::Regs(rs.iter().fold(0, |m, r| m | 1 << r.number()));
    [
        regs(&[Reg::R4]),
        regs(&[Reg::R4, Reg::Lr]),
        regs(&[Reg::R0, Reg::R1, Reg::R2, Reg::R3, Reg::R4, Reg::R5, Reg::R6, Reg::R7]),
        regs(&[Reg::R4, Reg::R5, Reg::Pc]),
        regs(&[Reg::R4, Reg::R8]),
        regs(&[Reg::R4, Reg::R11, Reg::Lr]),
        List::Doubles { first: V::V16, count: 8 },
        List::Doubles { first: V::V0, count: 1 },
        List::Doubles { first: V::V1, count: 1 },
    ]
    .map(Operand::List)
    .to_vec()
}
