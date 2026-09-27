use vun_inline::{ClassId, FormId, OpId, RegRef};
use vun_isa::{Blocks, ChoiceId, Fixed, Given, Misfit, Why};

use crate::{Choice, Class, Inst, Mask, Op, Operand, Reg, Special, Thumb};

type G = Given<u32, u32, u32>;

fn reg(class: Class, number: u8) -> G {
    Given::Reg { reg: RegRef { class: ClassId(class as u8), number }, size: None }
}

fn fit(op: Op, givens: &[G]) -> Result<(), Misfit> {
    let id = OpId(u16::try_from(Op::ALL.iter().position(|&o| o == op).expect("an op")).expect("few"));
    Thumb::fit(id, FormId(0), Fixed::default(), givens).map(drop)
}

#[test]
fn special_registers_are_read_into_general_ones() {
    assert_eq!(fit(Op::Mrs, &[reg(Class::General, 0), reg(Class::Special, Special::Primask as u8)]), Ok(()));
    let swapped = fit(Op::Mrs, &[reg(Class::Special, 0), reg(Class::General, 0)]);
    assert_eq!(swapped, Err(Misfit::Operand { at: 0, why: Why::Class }));
}

#[test]
fn a_choice_fits_the_positions_of_its_own_kind() {
    let mask = Given::Choice { of: ChoiceId(Choice::Mask as u8), index: Mask::I as u8 };
    assert_eq!(fit(Op::Cpsid, std::slice::from_ref(&mask)), Ok(()));
    assert_eq!(fit(Op::Dsb, &[mask]), Err(Misfit::Operand { at: 0, why: Why::Kind }));
}

#[test]
fn instructions_are_listed_as_gnu_as_reads_them() {
    let (op, form) = (Op::Mrs, &Op::Mrs.forms()[0]);
    let mrs = Inst::new(op, form, &[Operand::Reg(Reg::R0), Operand::Special(Special::Primask)]);
    assert_eq!(mrs.to_string(), "mrs r0, primask");
    let cpsid = Inst::new(Op::Cpsid, &Op::Cpsid.forms()[0], &[Operand::Mask(Mask::I)]);
    assert_eq!(cpsid.to_string(), "cpsid i");
}
