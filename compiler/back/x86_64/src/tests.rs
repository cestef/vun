//! Every form of the table, over operands covering its encodings' cases (REX-extended and byte
//! registers, SIB bases, displacements of each width, immediates at their bounds), decoded by an
//! independent decoder (yaxpeax-x86): the bytes are one instruction, and its text is the
//! printer's.

use yaxpeax_arch::{Decoder, LengthedInstruction, U8Reader};
use yaxpeax_x86::long_mode::InstDecoder;

use crate::{Asm, Cc, Emit, Kind, Mem, Op, Operand, Reg, Size, Xmm};

type Sym = &'static str;

const REGS: [Reg; 5] = [Reg::Rax, Reg::Rsi, Reg::Rbp, Reg::R9, Reg::R12];
const XMMS: [Xmm; 2] = [Xmm(1), Xmm(9)];
const MEMS: [Mem<Sym>; 5] = [
    Mem::at(Reg::Rbp, -8),
    Mem::at(Reg::Rsp, 0),
    Mem::at(Reg::R13, 0),
    Mem::at(Reg::R12, 300),
    Mem::at(Reg::Rax, -129),
];
const IMMS: [i64; 7] = [0, -1, 127, 128, -32768, 0xffff_ffff, 0x1234_5678_9abc_def0];
const CCS: [Cc; 3] = [Cc::B, Cc::Nz, Cc::G];

/// The operands tried at a position of `kind`.
fn samples(kind: Kind) -> Vec<Operand<Sym>> {
    let regs = || REGS.iter().map(|&r| Operand::Reg(r));
    let mems = || MEMS.iter().map(|&m| Operand::Mem(m));
    let xmms = || XMMS.iter().map(|&x| Operand::Xmm(x));
    match kind {
        Kind::R | Kind::Rn(_) => regs().collect(),
        Kind::M => mems().collect(),
        Kind::Rm | Kind::Rmn(_) => regs().chain(mems()).collect(),
        Kind::X => xmms().collect(),
        Kind::Xmn(_) => xmms().chain(mems()).collect(),
        Kind::Cl => vec![Operand::Reg(Reg::Rcx)],
        Kind::I8 | Kind::Iz | Kind::U32 | Kind::I64 => IMMS.iter().map(|&i| Operand::Imm(i)).collect(),
        Kind::Rel => vec![Operand::Sym("f")],
        Kind::Cond => CCS.iter().map(|&c| Operand::Cond(c)).collect(),
    }
}

/// Every combination of `kinds`' samples.
fn combinations(kinds: impl Iterator<Item = Kind>) -> Vec<Vec<Operand<Sym>>> {
    kinds.fold(vec![Vec::new()], |acc, k| {
        acc.iter()
            .flat_map(|prefix| {
                samples(k).into_iter().map(move |o| {
                    let mut next = prefix.clone();
                    next.push(o);
                    next
                })
            })
            .collect()
    })
}

#[test]
fn every_form_decodes_as_one_instruction_printed_alike() {
    let decoder = InstDecoder::default();
    let mut checked = 0;
    let mut differ = Vec::new();
    for &op in Op::ALL {
        for form in op.forms() {
            for size in Size::ALL.into_iter().filter(|&s| form.sizes.contains(s)) {
                for operands in combinations(form.kinds()) {
                    // A combination another form of the instruction takes first is that form's.
                    if op.form(size, &operands) != Some(form) {
                        continue;
                    }
                    let mut asm = Asm::<Sym>::listed();
                    asm.emit(op, size, &operands);
                    let code = asm.finish().unwrap();
                    let ours = match code.listing[0] {
                        vun_asm::Line::Inst { inst, .. } => inst,
                        vun_asm::Line::Label(_) => unreachable!("one instruction"),
                    };
                    let mut reader = U8Reader::new(&code.bytes);
                    let theirs = decoder
                        .decode(&mut reader)
                        .unwrap_or_else(|e| panic!("{ours}: {e} {:02x?}", code.bytes));
                    assert_eq!(
                        theirs.len().to_const(),
                        code.bytes.len() as u64,
                        "{ours}: {:02x?}",
                        code.bytes
                    );
                    // yaxpeax-x86 writes a string instruction's implicit operands, and decodes
                    // `movd xmm, r/m32` (`66 0f 6e`, no REX.W) as `movq`: those bytes are
                    // checked by `the_shortest_form_is_chosen`.
                    let known = op == Op::Movsb
                        || op == Op::Movd && size == Size::B32 && matches!(operands[0], Operand::Xmm(_));
                    if !form.kinds().any(|k| k == Kind::Rel)
                        && !known
                        && ours.to_string() != theirs.to_string()
                    {
                        differ.push(format!("{ours}  |  {theirs}"));
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(
        differ.is_empty(),
        "{} differ:\n{}",
        differ.len(),
        differ.iter().step_by(differ.len().div_ceil(80).max(1)).cloned().collect::<Vec<_>>().join("\n")
    );
    assert!(checked > 1000, "{checked}");
}

#[test]
fn the_shortest_form_is_chosen() {
    let bytes = |op, size, operands: &[Operand<Sym>]| {
        let mut asm = Asm::<Sym>::default();
        asm.emit(op, size, operands);
        asm.finish().unwrap().bytes
    };
    assert_eq!(bytes(Op::Mov, Size::B64, &[Reg::Rax.into(), 1.into()]), [0xb8, 1, 0, 0, 0]);
    assert_eq!(
        bytes(Op::Mov, Size::B64, &[Reg::R8.into(), (-1).into()]),
        [0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        bytes(Op::Mov, Size::B64, &[Reg::Rcx.into(), (1 << 40).into()]),
        [0x48, 0xb9, 0, 0, 0, 0, 0, 1, 0, 0]
    );
    assert_eq!(bytes(Op::Sub, Size::B64, &[Reg::Rsp.into(), 16.into()]), [0x48, 0x83, 0xec, 0x10]);
    assert_eq!(bytes(Op::Cmp, Size::B8, &[Reg::Rsi.into(), Reg::Rax.into()]), [0x40, 0x38, 0xc6]);
    assert_eq!(bytes(Op::Movd, Size::B32, &[Xmm(1).into(), Reg::Rax.into()]), [0x66, 0x0f, 0x6e, 0xc8]);
}

#[test]
fn a_rip_relative_operand_counts_from_the_end_of_the_instruction() {
    let mut asm = Asm::<Sym>::default();
    asm.emit(Op::Cmp, Size::B32, &[Mem::rip("d", vun_asm::Via::Direct).into(), 1000.into()]);
    let code = asm.finish().unwrap();
    // The displacement is followed by four bytes of immediate.
    assert_eq!(code.relocs[0].addend, -8);
    assert_eq!(code.relocs[0].offset, 2);
}

#[test]
fn every_form_of_an_instruction_uses_the_same_implicit_registers() {
    for &op in Op::ALL {
        assert!(op.forms().iter().all(|f| f.implicit == op.implicit()), "{op:?}");
    }
}
