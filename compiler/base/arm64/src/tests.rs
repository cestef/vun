use vun_inline::{Base, ClassId, Disp, FormId, Mem, OpId, RegRef};
use vun_isa::{Blocks, Fixed, Given, Misfit, Why};

use crate::{Arm64, Barrier, Class, Op, Reg, Size};

type G = Given<u32, u32, u32>;

fn reg(r: Reg, size: Size) -> G {
    let reg = RegRef { class: ClassId(Class::General as u8), number: r as u8 };
    Given::Reg { reg, size: Some(size.id()) }
}

/// The first form of `op` that takes `givens`, or the nearest miss.
fn fit(op: Op, givens: &[G]) -> Result<Size, Misfit> {
    let id = OpId(u16::try_from(Op::ALL.iter().position(|&o| o == op).expect("an op")).expect("few"));
    let mut miss = Misfit::Count;
    for form in 0..op.forms().len() {
        match Arm64::fit(id, FormId(u16::try_from(form).expect("few")), Fixed::default(), givens) {
            Ok(fit) => return Ok(Size::of(fit.size)),
            Err(m) => miss = m,
        }
    }
    Err(miss)
}

#[test]
fn register_names_give_the_width() {
    assert_eq!(
        fit(Op::Add, &[reg(Reg::X0, Size::B64), reg(Reg::X1, Size::B64), Given::Imm(Some(3))]),
        Ok(Size::B64)
    );
    assert_eq!(
        fit(Op::Add, &[reg(Reg::X0, Size::B32), reg(Reg::X1, Size::B32), Given::Imm(Some(3))]),
        Ok(Size::B32)
    );
    let mixed = fit(Op::Add, &[reg(Reg::X0, Size::B32), reg(Reg::X1, Size::B64), Given::Imm(Some(3))]);
    assert!(matches!(mixed, Err(Misfit::Operand { why: Why::Kind, .. })), "{mixed:?}");
}

#[test]
fn memory_is_taken_in_the_mode_of_its_position() {
    let base = Some(Base::Reg(RegRef { class: ClassId(Class::General as u8), number: Reg::X1 as u8 }));
    let at = |n| Given::Mem(Mem { base: base.clone(), index: None, disp: Disp::Bytes(n) });
    assert_eq!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(8)]), Ok(Size::B64));
    assert!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(1 << 20)]).is_err());
}

#[test]
fn a_barrier_is_a_named_choice() {
    let ish = u8::try_from(Barrier::ALL.iter().position(|&b| b == Barrier::Ish).expect("ish")).expect("few");
    assert!(fit(Op::Dmb, &[Given::Choice(ish)]).is_ok());
    assert!(fit(Op::Dmb, &[Given::Imm(Some(0))]).is_err());
}
