//! Fields written later: branches to labels, patched when the function ends, and relocations.

use vun_asm::{Isa, RelocKind, Unpatched, Via};

use super::{Sym, decode};
use crate::{Arm64, Asm, Cc, Emit, Fixup, Mem, Op, Operand, Reg, Size};

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks(4).map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect()
}

#[test]
fn branches_to_labels_count_instructions_both_ways() {
    let mut asm = Asm::<Sym>::default();
    let back = asm.label();
    let ahead = asm.label();
    asm.bind(back);
    asm.emit(Op::B, Size::B64, &[ahead.into()]);
    asm.emit(Op::Bcond, Size::B64, &[Cc::Ne.into(), back.into()]);
    asm.emit(Op::Cbz, Size::B32, &[Reg::X3.into(), ahead.into()]);
    asm.bind(ahead);
    let code = asm.finish().unwrap();
    let texts: Vec<String> = words(&code.bytes).into_iter().map(|w| decode(w).unwrap().to_string()).collect();
    assert_eq!(texts, ["b $+0xc", "b.ne $-0x4", "cbz w3, $+0x4"]);
}

#[test]
fn a_conditional_branch_reaches_a_mebibyte() {
    let reach = |gap: usize| {
        let mut asm = Asm::<Sym>::default();
        let label = asm.label();
        asm.emit(Op::Bcond, Size::B64, &[Cc::Eq.into(), label.into()]);
        asm.put(&vec![0; gap]);
        asm.bind(label);
        asm.finish().map(|_| ())
    };
    assert_eq!(reach((1 << 20) - 8), Ok(()));
    assert_eq!(reach(1 << 20), Err(Unpatched::Range { at: 0, distance: (1 << 20) + 4 }));
}

#[test]
fn a_jump_reaches_128_mebibytes() {
    let mut code = vec![0; 4];
    assert_eq!(Arm64::patch(Fixup::Imm26, &mut code, 0, (1 << 27) - 4), Ok(()));
    assert_eq!(words(&code), [(1 << 25) - 1]);
    let far = Arm64::patch(Fixup::Imm26, &mut [0; 4], 0, 1 << 27);
    assert_eq!(far, Err(Unpatched::Range { at: 0, distance: 1 << 27 }));
    assert!(!Fixup::Imm19.reaches(-(1 << 20) - 4) && Fixup::Imm19.reaches(-(1 << 20)));
}

#[test]
fn addresses_and_calls_are_relocated() {
    let mut asm = Asm::<Sym>::default();
    asm.emit(Op::Adrp, Size::B64, &[Reg::X0.into(), Operand::Page("data", Via::Direct)]);
    asm.emit(Op::Add, Size::B64, &[Reg::X0.into(), Reg::X0.into(), Operand::Lo12("data")]);
    asm.emit(Op::Adrp, Size::B64, &[Reg::X1.into(), Operand::Page("ext", Via::Got)]);
    let got = Mem::Lo12 { base: Reg::X1, target: "ext", via: Via::Got };
    asm.emit(Op::Ldr, Size::B64, &[Reg::X1.into(), got.into()]);
    let word = Mem::Lo12 { base: Reg::X0, target: "data", via: Via::Direct };
    asm.emit(Op::Str, Size::B32, &[Reg::Zr.into(), word.into()]);
    asm.emit(Op::Bl, Size::B64, &[Operand::Sym("f")]);
    asm.emit(Op::B, Size::B64, &[Operand::Sym("g")]);
    asm.emit(Op::Bcond, Size::B64, &[Cc::Lo.into(), Operand::Sym("h")]);
    let code = asm.finish().unwrap();
    let relocs: Vec<(u64, Sym, RelocKind, i64)> =
        code.relocs.iter().map(|r| (r.offset, r.target, r.kind, r.addend)).collect();
    assert_eq!(
        relocs,
        [
            (0, "data", RelocKind::Page21, 0),
            (4, "data", RelocKind::PageOffset12, 0),
            (8, "ext", RelocKind::GotPage21, 0),
            (12, "ext", RelocKind::GotPageOffset64, 0),
            (16, "data", RelocKind::PageOffset32, 0),
            (20, "f", RelocKind::Call26, 0),
            (24, "g", RelocKind::Jump26, 0),
            (28, "h", RelocKind::Branch19, 0),
        ]
    );
    let texts: Vec<String> = words(&code.bytes).into_iter().map(|w| decode(w).unwrap().to_string()).collect();
    assert_eq!(
        texts,
        [
            "adrp x0, $+0x0",
            "add x0, x0, #0x0",
            "adrp x1, $+0x0",
            "ldr x1, [x1]",
            "str wzr, [x0]",
            "bl $+0x0",
            "b $+0x0",
            "b.lo $+0x0"
        ]
    );
}

#[test]
fn only_a_64_bit_load_reads_the_global_offset_table() {
    let got = Mem::Lo12 { base: Reg::X1, target: "ext", via: Via::Got };
    assert!(Op::Ldr.form(Size::B32, &[Reg::X1.into(), got.into()]).is_none());
}
