use vun_inline::{Base, ClassId, Disp, FormId, Mem, OpId, RegRef};
use vun_isa::{Blocks, Fixed, Given, Misfit, Why, Writeback};

use crate::{Class, Op, Reg, Size, X86_64};

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
        match X86_64::fit(id, FormId(u16::try_from(form).expect("few")), Fixed::default(), givens) {
            Ok(fit) => return Ok(Size::of(fit.size)),
            Err(m) => miss = m,
        }
    }
    Err(miss)
}

#[test]
fn registers_give_the_instruction_its_width() {
    assert_eq!(fit(Op::Mov, &[reg(Reg::Rax, Size::B64), Given::Imm(Some(1))]), Ok(Size::B64));
    assert_eq!(fit(Op::Add, &[reg(Reg::Rcx, Size::B32), reg(Reg::Rdx, Size::B32)]), Ok(Size::B32));
    let mixed = fit(Op::Add, &[reg(Reg::Rcx, Size::B32), reg(Reg::Rdx, Size::B64)]);
    assert!(matches!(mixed, Err(Misfit::Operand { why: Why::Kind, .. })), "{mixed:?}");
}

#[test]
fn memory_and_an_immediate_give_no_width() {
    let base = Some(Base::Reg(RegRef { class: ClassId(Class::General as u8), number: Reg::Rax as u8 }));
    let givens: [G; 2] = [
        Given::Mem { mem: Mem { base, index: None, disp: Disp::Bytes(0) }, writeback: Writeback::No },
        Given::Imm(Some(1)),
    ];
    let mov = OpId(u16::try_from(Op::ALL.iter().position(|&o| o == Op::Mov).expect("mov")).expect("few"));
    let misses: Vec<_> = (0..Op::Mov.forms().len())
        .map(|f| X86_64::fit(mov, FormId(u16::try_from(f).expect("few")), Fixed::default(), &givens))
        .collect();
    assert!(misses.iter().all(Result::is_err), "{misses:?}");
    assert!(misses.contains(&Err(Misfit::Unsized)), "{misses:?}");
}

#[test]
fn an_immediate_is_a_value_the_operation_holds() {
    let wide = fit(Op::Add, &[reg(Reg::Rax, Size::B8), Given::Imm(Some(300))]);
    assert!(matches!(wide, Err(Misfit::Operand { at: 1, why: Why::Wide })), "{wide:?}");
    assert_eq!(fit(Op::Add, &[reg(Reg::Rax, Size::B8), Given::Imm(Some(-1))]), Ok(Size::B8));
}

#[test]
fn a_register_of_another_class_is_refused() {
    let xmm = Given::Reg { reg: RegRef { class: ClassId(Class::Sse as u8), number: 0 }, size: None };
    let class = fit(Op::Add, &[reg(Reg::Rax, Size::B64), xmm]);
    assert!(matches!(class, Err(Misfit::Operand { at: 1, why: Why::Class | Why::Kind })), "{class:?}");
}
