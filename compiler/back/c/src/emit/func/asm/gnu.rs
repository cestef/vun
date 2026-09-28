//! What GNU C writes of each instruction set's blocks: its syntax, the registers a binding may
//! take, and each instruction's text as its assembler lists it; and the symbols and lines of a
//! block as the C statement names them.

use std::fmt;

use vun_asm::{AsmOperand, Blocks, Place, Via};
use vun_asm_arm64::Arm64;
use vun_asm_x86_64::{Syntax, X86_64};
use vun_inline::{Inst, Line, Rebind};
use vun_isa_thumb::Thumb;
use vun_lir::{Address, Output, Value};

/// A line of a block, as the text GNU C assembles names its label: unique to each `__asm__`
/// statement by `%=`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Labelled(pub(super) Line);

impl fmt::Display for Labelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".Lvun%=_{}", self.0.0)
    }
}

pub(super) type Block = vun_inline::Block<Value, Output, Labelled>;

/// What GNU C writes of an instruction set's blocks.
pub(super) trait Gnu: Blocks {
    /// The lines before and after a block's, setting the assembler's syntax to the listing's and
    /// back.
    const ENTER: &'static [&'static str];
    const LEAVE: &'static [&'static str];

    /// The registers a binding may take: the argument registers of the C convention.
    fn free(convention: vun_target::Convention) -> &'static [Self::Reg];

    /// A register's name, as the assembler reads it.
    fn name(r: Self::Reg) -> String;

    /// The text of `inst` over `operands`.
    fn text(inst: &Inst<Value, Output, Labelled>, operands: &[Self::Operand<Labelled>]) -> String;

    /// The text of the move of the stack pointer into `dst`.
    fn read_stack(dst: Self::Reg) -> String;
}

impl Gnu for X86_64 {
    const ENTER: &'static [&'static str] = &[".intel_syntax noprefix"];
    const LEAVE: &'static [&'static str] = &[".att_syntax prefix"];

    fn free(convention: vun_target::Convention) -> &'static [vun_asm_x86_64::Reg] {
        let mut conventions = vun_asm_x86_64::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, c)| c.registers.int_args)
    }

    fn name(r: vun_asm_x86_64::Reg) -> String {
        r.name(vun_asm_x86_64::Size::B64).to_owned()
    }

    fn text(inst: &Inst<Value, Output, Labelled>, operands: &[vun_asm_x86_64::Operand<Labelled>]) -> String {
        let op = vun_asm_x86_64::Op::entry(inst.op, inst.form).0;
        let size = vun_asm_x86_64::Size::of(inst.size);
        vun_asm_x86_64::Inst::new(op, size, operands).text(Syntax::Gnu).to_string()
    }

    fn read_stack(dst: vun_asm_x86_64::Reg) -> String {
        let operands: [vun_asm_x86_64::Operand<Labelled>; 2] = [dst.into(), vun_asm_x86_64::Reg::Rsp.into()];
        let size = vun_asm_x86_64::Size::B64;
        vun_asm_x86_64::Inst::new(vun_asm_x86_64::Op::Mov, size, &operands).text(Syntax::Gnu).to_string()
    }
}

impl Gnu for Arm64 {
    const ENTER: &'static [&'static str] = &[];
    const LEAVE: &'static [&'static str] = &[];

    fn free(convention: vun_target::Convention) -> &'static [vun_asm_arm64::Reg] {
        let mut conventions = vun_asm_arm64::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, c)| c.registers.int_args)
    }

    fn name(r: vun_asm_arm64::Reg) -> String {
        r.name(vun_asm_arm64::Size::B64).to_string()
    }

    fn text(inst: &Inst<Value, Output, Labelled>, operands: &[vun_asm_arm64::Operand<Labelled>]) -> String {
        let op = vun_asm_arm64::Op::entry(inst.op, inst.form).0;
        vun_asm_arm64::Inst::new(op, vun_asm_arm64::Size::of(inst.size), operands).to_string()
    }

    fn read_stack(dst: vun_asm_arm64::Reg) -> String {
        let operands: [vun_asm_arm64::Operand<Labelled>; 2] = [dst.into(), vun_asm_arm64::Reg::Sp.into()];
        vun_asm_arm64::Inst::new(vun_asm_arm64::Op::Mov, vun_asm_arm64::Size::B64, &operands).to_string()
    }
}

/// A block's symbols, which GNU C cannot name, refused; its lines named as labels.
pub(super) struct Unlinked;

impl Rebind<Value, Output, Address> for Unlinked {
    type Value = Value;
    type Place = Output;
    type Symbol = Labelled;
    type Error = AsmOperand;

    fn value(&mut self, value: Value) -> Result<Value, AsmOperand> {
        Ok(value)
    }

    fn place(&mut self, place: Output) -> Result<Output, AsmOperand> {
        Ok(place)
    }

    fn symbol(&mut self, _: Address) -> Result<Labelled, AsmOperand> {
        Err(AsmOperand::Symbol)
    }
}

/// Where a block's operands are in the C statement: its bindings in the registers given them, in
/// order, and its lines at their labels.
pub(super) struct Given<I: Blocks> {
    pub(super) regs: std::vec::IntoIter<I::Reg>,
}

impl<I: Blocks> Place<I, Labelled> for Given<I> {
    fn bound(&mut self) -> I::Reg {
        self.regs.next().expect("a register for each binding")
    }

    fn line(&self, line: Line) -> I::Operand<Labelled> {
        I::target(Labelled(line))
    }

    /// Never asked: a block naming a symbol is refused.
    fn via(&self, _: Labelled) -> Via {
        Via::Direct
    }
}

impl Gnu for Thumb {
    const ENTER: &'static [&'static str] = &[];
    const LEAVE: &'static [&'static str] = &[];

    fn free(convention: vun_target::Convention) -> &'static [vun_isa_thumb::Reg] {
        let mut conventions = vun_isa_thumb::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, args)| args)
    }

    fn name(r: vun_isa_thumb::Reg) -> String {
        r.name().to_owned()
    }

    fn text(inst: &Inst<Value, Output, Labelled>, operands: &[vun_isa_thumb::Operand]) -> String {
        let (op, form) = vun_isa_thumb::Op::entry(inst.op, inst.form);
        vun_isa_thumb::Inst::new(op, form, operands).to_string()
    }

    fn read_stack(dst: vun_isa_thumb::Reg) -> String {
        let op = vun_isa_thumb::Op::Mov;
        let operands = [dst.into(), vun_isa_thumb::Reg::Sp.into()];
        vun_isa_thumb::Inst::new(op, &op.forms()[0], &operands).to_string()
    }
}
