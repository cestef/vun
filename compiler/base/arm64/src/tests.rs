use vun_inline::{Base, ClassId, Disp, FormId, Mem, OpId, RegRef};
use vun_isa::{Blocks, ChoiceId, Fixed, Given, Misfit, Why, Writeback};

use crate::{Arm64, Barrier, Choice, Class, Op, Reg, Size};

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
    let mem = |n| Mem { base: base.clone(), index: None, disp: Disp::Bytes(n) };
    let at = |n, writeback| Given::Mem { mem: mem(n), writeback };
    assert_eq!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(8, Writeback::No)]), Ok(Size::B64));
    assert!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(1 << 20, Writeback::No)]).is_err());
    // Writing back takes only the forms of its mode, whose offsets are -256 to 255.
    assert_eq!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(-8, Writeback::Pre)]), Ok(Size::B64));
    assert!(fit(Op::Ldr, &[reg(Reg::X0, Size::B64), at(4096, Writeback::Post)]).is_err());
}

#[test]
fn a_barrier_is_a_named_choice() {
    let ish = u8::try_from(Barrier::ALL.iter().position(|&b| b == Barrier::Ish).expect("ish")).expect("few");
    let barrier = ChoiceId(Choice::Barrier as u8);
    assert!(fit(Op::Dmb, &[Given::Choice { of: barrier, index: ish }]).is_ok());
    // A choice of another kind is refused even where its index would be a barrier's.
    assert!(fit(Op::Dmb, &[Given::Choice { of: ChoiceId(Choice::Sys as u8), index: 0 }]).is_err());
    assert!(fit(Op::Dmb, &[Given::Imm(Some(0))]).is_err());
}

#[test]
fn an_access_width_the_spelling_gives_reads_into_a_w_register() {
    let base = Some(Base::Reg(RegRef { class: ClassId(Class::General as u8), number: Reg::X1 as u8 }));
    let mem = Given::Mem { mem: Mem { base, index: None, disp: Disp::Bytes(3) }, writeback: Writeback::No };
    let fixed = Fixed { cond: None, size: Some(Size::B8.id()) };
    let fitted = (0..Op::Ldr.forms().len()).find_map(|f| {
        let form = FormId(u16::try_from(f).expect("few"));
        Arm64::fit(OpId(Op::Ldr as u16), form, fixed, &[reg(Reg::X0, Size::B32), mem.clone()]).ok()
    });
    assert_eq!(fitted.map(|f| f.size), Some(Size::B8.id()));
}
