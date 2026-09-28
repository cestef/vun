use vun_isa::word::Field;

use crate::{Branch, Fixup, Imm, Kind, Mem, Op, Operand, Place, Reg, Size};

type O = Operand<u32>;

fn encode(op: Op, operands: &[O]) -> Option<u32> {
    op.select(Size::B32, operands).map(|(_, word)| word)
}

#[test]
fn a_modified_immediate_is_a_byte_shifted_or_repeated() {
    let modified = |v: i64| Imm::Modified.encode(v);
    // The fields `i` at 26, `imm3` at 12, `imm8` at 0 of the twelve bits.
    assert_eq!(modified(0xab), Some(0xab));
    assert_eq!(modified(0x00ab_00ab), Some(0x10ab));
    assert_eq!(modified(0xab00_ab00), Some(0x20ab));
    assert_eq!(modified(0xabab_abab), Some(0x30ab));
    // 0x3fc is 0xff rotated right by 30: 30 in the top five bits, the byte's low seven below.
    assert_eq!(modified(0x3fc), Some(0x0400_707f));
    assert_eq!(modified(0x101), None);
    assert_eq!(modified(-1), Some(0x30ff));
}

#[test]
fn shifts_are_placed_in_one_field_or_two() {
    for amount in 0..32 {
        let wide = Place::Wide.shift(amount);
        assert_eq!(Place::Wide.amount(wide), amount);
    }
    assert_eq!(Imm::Shr(Place::Narrow).encode(32), Some(0));
    assert_eq!(Imm::Shr(Place::Narrow).encode(0), None);
}

#[test]
fn the_first_form_fitting_is_the_shortest() {
    let (r0, r1, r8) = (Reg::R0.into(), Reg::R1.into(), Reg::R8.into());
    // `adds r0, r1, #1` has a 16-bit form; `r8` is not a low register.
    assert_eq!(encode(Op::Adds, &[r0, r1, O::Imm(1)]), Some(0x1c48));
    assert_eq!(encode(Op::Adds, &[r8, r1, O::Imm(1)]), Some(0xf111_0801));
    assert_eq!(encode(Op::Ldr, &[r0, O::Mem(Mem::at(Reg::R1, 4))]), Some(0x6848));
    assert_eq!(encode(Op::Ldr, &[r0, O::Mem(Mem::at(Reg::R1, -4))]), Some(0xf851_0c04));
    assert_eq!(encode(Op::Mov, &[r8, r1]), Some(0x4688));
}

#[test]
fn a_branch_reaches_as_far_as_its_field() {
    assert_eq!(Fixup::Jump.reach(0, 4), Some(0));
    assert_eq!(Fixup::Compare.reach(8, 4), None);
    let kind = Kind::Target(Branch::Jump);
    assert_eq!(kind.encode(&O::Sym(0), Size::B32), Some(0));
    // Back by 4 halfwords: every bit of the distance set, `J1` and `J2` too as `S` inverts them.
    assert_eq!(Fixup::Jump.fields(-4), 0x07ff_2ffc);
}
