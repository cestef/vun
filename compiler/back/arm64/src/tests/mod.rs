//! Every form of the table, over operands covering its encodings' cases (register 31 as the
//! stack pointer and the zero register, offsets at their bounds, immediates each encoding holds
//! and some it does not), decoded by an independent decoder: the word is one instruction, and its
//! text is the printer's.

mod fields;
mod links;
mod quirks;

use std::collections::HashMap;

use vun_asm::{Label, Line, Via};
use yaxpeax_arch::{Decoder, U8Reader};
use yaxpeax_arm::armv8::a64::{InstDecoder, Instruction};

use crate::{
    Asm, Barrier, Cc, Emit, Extend, Form, Inst, Kind, Mem, Mode, Op, Operand, Reg, Scale, Shift, ShiftKind, SysReg,
    Size, V,
};

type Sym = &'static str;

const REGS: [Reg; 5] = [Reg::X0, Reg::X17, Reg::X30, Reg::Sp, Reg::Zr];
const VS: [V; 2] = [V(1), V(31)];
const IMMS: [i64; 22] = [
    0,
    1,
    7,
    31,
    63,
    0xfff,
    0x1000,
    0xab_c000,
    0x1001,
    0xffff,
    0x1_0000,
    0x1234_0000,
    0xffff_0000,
    0x1234_0000_0000,
    -1,
    -2,
    -0x1_0001,
    0xff,
    0x0f0f_0f0f,
    0x5555_5555_5555_5555,
    0x7fff_ffff_ffff_fffe,
    i64::MIN,
];
const CCS: [Cc; 6] = [Cc::Eq, Cc::Ne, Cc::Hs, Cc::Lo, Cc::Gt, Cc::Le];
const OFFSETS: [i64; 11] = [0, 8, 1, -8, 255, -256, 256, 4095 * 8, 504, -512, 4096];
const SHIFTS: [(Reg, ShiftKind, u8); 5] = [
    (Reg::X17, ShiftKind::Lsl, 3),
    (Reg::X30, ShiftKind::Lsr, 17),
    (Reg::X0, ShiftKind::Asr, 31),
    (Reg::X17, ShiftKind::Ror, 5),
    (Reg::X0, ShiftKind::Lsl, 63),
];
const EXTENDS: [(Reg, Extend, u8); 7] = [
    (Reg::X17, Extend::Uxtw, 0),
    (Reg::X17, Extend::Sxtw, 0),
    (Reg::X0, Extend::Uxtb, 2),
    (Reg::X30, Extend::Sxtx, 4),
    (Reg::X17, Extend::Uxth, 1),
    (Reg::X0, Extend::Sxth, 0),
    (Reg::Zr, Extend::Sxtb, 3),
];
const INDEXES: [(Reg, Reg, Extend, Scale); 7] = [
    (Reg::X1, Reg::X2, Extend::Sxtw, Scale::One),
    (Reg::X1, Reg::X2, Extend::Uxtw, Scale::One),
    (Reg::X1, Reg::X2, Extend::LSL, Scale::One),
    (Reg::Sp, Reg::X30, Extend::LSL, Scale::Access),
    (Reg::X1, Reg::X17, Extend::Uxtw, Scale::Access),
    (Reg::X1, Reg::Zr, Extend::Sxtw, Scale::Access),
    (Reg::X1, Reg::X2, Extend::Sxtx, Scale::Access),
];

/// Memory operands of every mode.
fn mems() -> Vec<Mem<Sym>> {
    let bases = [Reg::X1, Reg::Sp];
    let mut all = Vec::new();
    for base in bases {
        for offset in OFFSETS {
            all.extend([Mem::Off { base, offset }, Mem::Pre { base, offset }, Mem::Post { base, offset }]);
        }
    }
    all.extend(INDEXES.map(|(base, index, extend, scale)| Mem::Index { base, index, extend, scale }));
    all.extend([Via::Direct, Via::Got].map(|via| Mem::Lo12 { base: Reg::X1, target: "f", via }));
    all
}

/// The operands tried at a position of `kind`.
fn samples(kind: Kind, label: Label) -> Vec<Operand<Sym>> {
    let regs = || REGS.iter().map(|&r| Operand::Reg(r));
    match kind {
        Kind::Gpr(..) => regs().collect(),
        Kind::Fpr(_) => VS.iter().map(|&v| Operand::V(v)).collect(),
        Kind::Imm(_) => IMMS.iter().map(|&i| Operand::Imm(i)).collect(),
        Kind::Shifted(_) => {
            let shifted = SHIFTS.map(|(r, kind, amount)| Operand::Shifted(r, Shift { kind, amount }));
            regs().chain(shifted).collect()
        }
        Kind::Extended => {
            regs().chain(EXTENDS.map(|(r, e, amount)| Operand::Extended(r, e, amount))).collect()
        }
        Kind::Mem(..) => mems().into_iter().map(Operand::Mem).collect(),
        Kind::Cond(_) => CCS.iter().map(|&c| Operand::Cond(c)).collect(),
        Kind::Target(_) => vec![Operand::Label(label), Operand::Sym("f")],
        Kind::Page => vec![Operand::Page("f", Via::Direct), Operand::Page("f", Via::Got)],
        Kind::Lo12 => vec![Operand::Lo12("f")],
        Kind::Barrier => Barrier::ALL.map(Operand::Barrier).to_vec(),
        Kind::Sys => SysReg::ALL.map(Operand::Sys).to_vec(),
    }
}

/// Every combination of the samples at `kinds`.
fn combinations(kinds: impl Iterator<Item = Kind>, label: Label) -> Vec<Vec<Operand<Sym>>> {
    kinds.fold(vec![Vec::new()], |acc, k| {
        acc.iter()
            .flat_map(|prefix| {
                samples(k, label).into_iter().map(move |o| {
                    let mut next = prefix.clone();
                    next.push(o);
                    next
                })
            })
            .collect()
    })
}

/// `op` at `size` over `operands`, a label bound after it: its word and its instruction.
fn assemble(op: Op, size: Size, operands: &[Operand<Sym>]) -> (u32, Inst<Sym>) {
    let mut asm = Asm::<Sym>::listed();
    let label = asm.label();
    asm.emit(op, size, operands);
    asm.bind(label);
    let code = asm.finish().unwrap();
    let Line::Inst { inst, .. } = code.listing[0] else { unreachable!("one instruction") };
    (u32::from_le_bytes(code.bytes[..4].try_into().unwrap()), inst)
}

fn decode(word: u32) -> Option<Instruction> {
    let bytes = word.to_le_bytes();
    InstDecoder::default().decode(&mut U8Reader::new(&bytes)).ok()
}

/// Instructions sharing encodings: an instruction and its aliases, which a decoder may prefer.
const ALIASES: &[&[Op]] = &[
    &[Op::Add, Op::Mov],
    &[Op::Orr, Op::Mov],
    &[Op::Sxt, Op::Sxtb, Op::Sxth, Op::Sxtw, Op::Mov, Op::Orr],
    &[Op::Uxt, Op::Uxtb, Op::Uxth, Op::Mov, Op::Orr],
    &[Op::Orn, Op::Mvn],
    &[Op::Sub, Op::Neg],
    &[Op::Subs, Op::Negs, Op::Cmp],
    &[Op::Adds, Op::Cmn],
    &[Op::Ands, Op::Tst],
    &[Op::Madd, Op::Mul],
    &[Op::Msub, Op::Mneg],
    &[Op::Csinc, Op::Cinc, Op::Cset],
    &[Op::Csinv, Op::Cinv, Op::Csetm],
    &[Op::Csneg, Op::Cneg],
    &[Op::Ldadd, Op::Stadd],
    &[Op::Ldaddl, Op::Staddl],
];

/// An emitted instruction, its word and its text.
struct Emitted {
    op: Op,
    size: Size,
    operands: Vec<Operand<Sym>>,
    form: &'static Form,
    word: u32,
    text: String,
}

impl Emitted {
    /// Whether a field of it is written later: by the linker, or when a label is bound.
    fn linked(&self) -> bool {
        self.form
            .args
            .iter()
            .any(|a| matches!(a.kind, Kind::Target(_) | Kind::Page | Kind::Lo12 | Kind::Mem(Mode::Lo12, _)))
    }
}

#[test]
fn every_form_decodes_as_one_instruction_printed_alike() {
    let label = Asm::<Sym>::default().label();
    let mut emitted = Vec::new();
    let mut texts: HashMap<u32, Vec<(Op, String)>> = HashMap::new();
    for &op in Op::ALL {
        for form in op.forms() {
            for size in form.sizes.iter().map(|s| s.size) {
                for operands in combinations(form.args.iter().map(|a| a.kind), label) {
                    // A combination another form of the instruction takes first is that form's.
                    if op.form(size, &operands) != Some(form) {
                        continue;
                    }
                    let (word, inst) = assemble(op, size, &operands);
                    let text = inst.to_string();
                    texts.entry(word).or_default().push((op, text.clone()));
                    emitted.push(Emitted { op, size, operands, form, word, text });
                }
            }
        }
    }
    let mut wrong = Vec::new();
    for e in &emitted {
        let decoded = decode(e.word).unwrap_or_else(|| panic!("{}: {:08x} does not decode", e.text, e.word));
        let theirs = decoded.to_string();
        // A field written later is still 0: the mnemonic is compared, the field by `links`.
        let alike =
            if e.linked() { e.text.split(' ').next() == theirs.split(' ').next() } else { theirs == e.text };
        // The decoder prefers an alias the table declares too, whose encoding is checked alike.
        let alias = |op: Op| op == e.op || ALIASES.iter().any(|a| a.contains(&op) && a.contains(&e.op));
        let ours: Vec<String> =
            texts[&e.word].iter().filter(|(op, _)| alias(*op)).map(|(_, t)| t.clone()).collect();
        if !alike && !ours.contains(&theirs) && !e.known_quirk(&decoded, &theirs, &ours) {
            wrong.push(format!("{:08x}: {} / {theirs}", e.word, e.text));
        }
    }
    assert!(wrong.is_empty(), "{} of {}:\n{}", wrong.len(), emitted.len(), wrong.join("\n"));
    assert!(emitted.len() > 5000, "{}", emitted.len());
}
