//! Immediates and offsets: which values each encoding holds.

use super::Sym;
use crate::{Bitmask, Kind, Mem, Mode, Op, Operand, Reg, Size, Width};

/// The pattern `N:immr:imms` encodes, as an operation of `bits` reads it, if it encodes one: the
/// Arm pseudocode's `DecodeBitMasks`, independent of `Bitmask::of`.
fn decode_bitmask(field: u32, bits: u32) -> Option<u64> {
    let (n, rotation, length) = (field >> 12 & 1, field >> 6 & 0x3f, field & 0x3f);
    let len = (n << 6 | (!length & 0x3f)).checked_ilog2()?;
    let size = 1u32.checked_shl(len).filter(|&s| s >= 2 && s <= bits)?;
    let ones = (length & (size - 1)) + 1;
    if ones == size {
        return None;
    }
    let mask = if size == 64 { u64::MAX } else { (1 << size) - 1 };
    let run = (1u64 << ones) - 1;
    let element = (run >> (rotation % size) | run << ((size - rotation % size) % size)) & mask;
    let mut value = 0;
    let mut at = 0;
    while at < bits {
        value |= element << at;
        at += size;
    }
    Some(if bits == 64 { value } else { value & 0xffff_ffff })
}

#[test]
fn every_bitmask_encodes_back_to_its_field() {
    for bits in [32, 64] {
        let mut count = 0;
        for field in 0..1 << 13 {
            let Some(value) = decode_bitmask(field, bits) else { continue };
            let encoded = Bitmask::of(value.cast_signed(), bits).map(Bitmask::bits);
            // Rotations past an element's size alias smaller ones; the value is what must agree.
            let back = encoded.and_then(|f| decode_bitmask(f, bits));
            assert_eq!(back, Some(value), "{field:013b} at {bits}");
            count += u32::from(encoded == Some(field));
        }
        // Each element size has size * (size - 1) patterns.
        let patterns: u32 =
            [2u32, 4, 8, 16, 32, 64].iter().filter(|&&s| s <= bits).map(|s| s * (s - 1)).sum();
        assert_eq!(count, patterns, "{bits}");
    }
}

#[test]
fn bitmasks_are_recognized() {
    let fits = |v: i64, bits| Bitmask::of(v, bits).is_some();
    for v in [0x5555_5555_5555_5555, 0xff, 0x0f0f_0f0f_0f0f_0f0f, i64::MIN | 1, -2, 0x00ff_00ff_00ff_00ff] {
        assert!(fits(v, 64), "{v:#x}");
    }
    for v in [0xffff_0000, 0x7fff_ffff, 0x8000_0001, -2] {
        assert!(fits(v, 32), "{v:#x}");
    }
    for v in [0, -1, 0x1234, 5, 0x0f0f_0f0f_0f0f_0f0e] {
        assert!(!fits(v, 64), "{v:#x}");
    }
    assert!(!fits(0xffff_ffff, 32));
    assert!(!fits(0xffff_ffff_0000_0000_u64.cast_signed(), 32));
}

/// The addressing mode `op` loads `x0` from `mem` in at `size`, if one instruction does.
fn mode(op: Op, size: Size, mem: Mem<Sym>) -> Option<Mode> {
    let form = op.form(size, &[Operand::Reg(Reg::X0), Operand::Mem(mem)])?;
    form.args.iter().find_map(|a| match a.kind {
        Kind::Mem(mode, Width::Inst) => Some(mode),
        _ => None,
    })
}

#[test]
fn offsets_fold_into_the_access_that_reaches_them() {
    let at = |offset| Mem::at(Reg::X1, offset);
    assert_eq!(mode(Op::Ldr, Size::B64, at(0)), Some(Mode::Scaled));
    assert_eq!(mode(Op::Ldr, Size::B64, at(32760)), Some(Mode::Scaled));
    assert_eq!(mode(Op::Ldr, Size::B64, at(32768)), None);
    assert_eq!(mode(Op::Ldr, Size::B64, at(-8)), Some(Mode::Unscaled));
    assert_eq!(mode(Op::Ldr, Size::B64, at(3)), Some(Mode::Unscaled));
    assert_eq!(mode(Op::Ldr, Size::B64, at(-257)), None);
    assert_eq!(mode(Op::Str, Size::B8, at(4095)), Some(Mode::Scaled));
    assert_eq!(mode(Op::Str, Size::B8, at(4096)), None);
    assert_eq!(mode(Op::Ldr, Size::B16, at(4097)), None);
    assert_eq!(mode(Op::Ldrs, Size::B32, at(16380)), Some(Mode::Scaled));
    let pair = |offset| {
        let form = Op::Ldp
            .form(Size::B64, &[Reg::X0.into(), Reg::X1.into(), Mem::<Sym>::at(Reg::Sp, offset).into()]);
        form.is_some()
    };
    assert!(pair(504) && pair(-512) && !pair(512) && !pair(4));
    let pre = Mem::<Sym>::Pre { base: Reg::Sp, offset: -16 };
    assert!(Op::Stp.form(Size::B64, &[Reg::X29.into(), Reg::X30.into(), pre.into()]).is_some());
}

#[test]
fn constants_fold_into_the_move_that_holds_them() {
    let form = |v: i64, size| Op::Mov.form::<Sym>(size, &[Reg::X0.into(), v.into()]).map(|f| f.base);
    let [_, _, wide, inverted, bitmask] = Op::Mov.forms() else { unreachable!("five forms") };
    let (wide, inverted, bitmask) = (wide.base, inverted.base, bitmask.base);
    assert_eq!(form(0x1_0000, Size::B64), Some(wide));
    assert_eq!(form(-2, Size::B64), Some(inverted));
    assert_eq!(form(0xffff_fffe, Size::B32), Some(inverted));
    assert_eq!(form(0x5555_5555_5555_5555, Size::B64), Some(bitmask));
    assert_eq!(form(0x1234_5678, Size::B64), None);
    let add = |v: i64| Op::Add.form::<Sym>(Size::B64, &[Reg::X0.into(), Reg::Sp.into(), v.into()]).is_some();
    assert!(add(0xfff) && add(0xfff_000) && !add(0x1001) && !add(-1));
}
