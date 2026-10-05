//! What GNU C writes of each instruction set's blocks: its syntax, the registers a binding may
//! take, and each instruction's text as its assembler lists it; and the symbols and lines of a
//! block as its text names them.

use std::fmt;

use vun_asm::{AsmOperand, Place, Registered, Via};
use vun_asm_arm64::Arm64;
use vun_asm_thumb::Thumb;
use vun_asm_x86_64::{Syntax, X86_64};
use vun_index::Idx;
use vun_inline::{ClassId, Inst, Line, Rebind, RegRef};
use vun_intern::Symbol;
use vun_lir::{Address, FuncId, Output, Value};
use vun_target::{Attributes, Name};

use crate::ast::Constraint;
use crate::emit::Emitter;

/// The object format whose symbols assembly names as C does.
const ELF: Name = Name("elf");

/// Where the labels of a block's lines are unique: in its `__asm__` statement, by the `%=` GNU C
/// expands in a statement with operands; or, in the plain `__asm__` of a naked function, by its
/// instruction in the unit.
#[derive(Clone, Copy, Debug)]
pub(super) enum Scope {
    Statement,
    Inst(FuncId, vun_lir::Inst),
}

/// A line of a block, as the text GNU C assembles names its label.
#[derive(Clone, Copy, Debug)]
pub(super) struct Labelled {
    pub(super) line: Line,
    pub(super) scope: Scope,
}

impl fmt::Display for Labelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.scope {
            Scope::Statement => write!(f, ".Lvun%=_{}", self.line.0),
            Scope::Inst(func, inst) => write!(f, ".Lvun{}_{}_{}", func.index(), inst.index(), self.line.0),
        }
    }
}

/// What a block's text names: a line, by its label, or a symbol, by its name in the object and
/// how an address reaches it.
#[derive(Clone, Copy, Debug)]
pub(super) enum Named {
    Line(Labelled),
    Symbol(Symbol, Via),
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Line(line) => line.fmt(f),
            Self::Symbol(symbol, _) => f.write_str(symbol.as_str()),
        }
    }
}

pub(super) type Block = vun_inline::Block<Value, Output, Named>;

/// What GNU C writes of an instruction set's blocks.
pub(super) trait Gnu: Registered {
    /// The lines before and after a block's, setting the assembler's syntax to the listing's and
    /// back.
    const ENTER: &'static [&'static str];
    const LEAVE: &'static [&'static str];

    /// The registers a call overwrites besides its arguments that C keeps for nothing else,
    /// which a binding may take once the arguments' are taken.
    const SCRATCH: &'static [Self::Reg];

    /// The argument registers of the C convention.
    fn args(convention: vun_target::Convention) -> &'static [Self::Reg];

    /// The registers the C convention passes floats in, which a float bound in one may take.
    fn floats(convention: vun_target::Convention) -> Vec<RegRef>;

    /// The name of the floating-point register `number` holding a value `size` wide, if it
    /// holds one that wide.
    fn float_name(number: u8, size: Self::Size) -> Option<String>;

    /// The class GNU C holds a float `size` wide in.
    fn float_class(size: Self::Size) -> Constraint;

    /// The registers a binding may take, in order: the arguments', then the scratch ones.
    fn free(convention: vun_target::Convention) -> impl Iterator<Item = Self::Reg> {
        Self::args(convention).iter().chain(Self::SCRATCH).copied()
    }

    /// A register's name, as the assembler reads it.
    fn name(r: Self::Reg) -> String;

    /// The text of `inst` over `operands`.
    fn text(inst: &Inst<Value, Output, Named>, operands: &[Self::Operand<Named>]) -> String;

    /// The text of the move of the stack pointer into `dst`.
    fn read_stack(dst: Self::Reg) -> String;
}

impl Gnu for X86_64 {
    const ENTER: &'static [&'static str] = &[".intel_syntax noprefix"];
    const LEAVE: &'static [&'static str] = &[".att_syntax prefix"];

    const SCRATCH: &'static [vun_asm_x86_64::Reg] =
        &[vun_asm_x86_64::Reg::Rax, vun_asm_x86_64::Reg::R10, vun_asm_x86_64::Reg::R11];

    fn args(convention: vun_target::Convention) -> &'static [vun_asm_x86_64::Reg] {
        let mut conventions = vun_asm_x86_64::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, c)| c.registers.int_args)
    }

    fn floats(convention: vun_target::Convention) -> Vec<RegRef> {
        let mut conventions = vun_asm_x86_64::conventions::CONVENTIONS.iter();
        let args =
            conventions.find(|(c, _)| *c == convention).map_or(&[][..], |(_, c)| c.registers.float_args);
        args.iter().map(|x| RegRef { class: vun_asm_x86_64::Class::Sse.id(), number: x.0 }).collect()
    }

    fn float_class(_: vun_asm_x86_64::Size) -> Constraint {
        Constraint('x')
    }

    /// An SSE register holds a float of either width.
    fn float_name(number: u8, _: vun_asm_x86_64::Size) -> Option<String> {
        Some(vun_asm_x86_64::Xmm(number).to_string())
    }

    fn name(r: vun_asm_x86_64::Reg) -> String {
        r.name(vun_asm_x86_64::Size::B64).to_owned()
    }

    fn text(inst: &Inst<Value, Output, Named>, operands: &[vun_asm_x86_64::Operand<Named>]) -> String {
        let op = vun_asm_x86_64::Op::entry(inst.op, inst.form).0;
        let size = vun_asm_x86_64::Size::of(inst.size);
        vun_asm_x86_64::Inst::new(op, size, operands).text(Syntax::Gnu).to_string()
    }

    fn read_stack(dst: vun_asm_x86_64::Reg) -> String {
        let operands: [vun_asm_x86_64::Operand<Named>; 2] = [dst.into(), vun_asm_x86_64::Reg::Rsp.into()];
        let size = vun_asm_x86_64::Size::B64;
        vun_asm_x86_64::Inst::new(vun_asm_x86_64::Op::Mov, size, &operands).text(Syntax::Gnu).to_string()
    }
}

impl Gnu for Arm64 {
    const ENTER: &'static [&'static str] = &[];
    const LEAVE: &'static [&'static str] = &[];

    /// `x16` and `x17` are the linker's, between a call and its callee.
    const SCRATCH: &'static [vun_asm_arm64::Reg] = {
        use vun_asm_arm64::Reg;
        &[Reg::X9, Reg::X10, Reg::X11, Reg::X12, Reg::X13, Reg::X14, Reg::X15]
    };

    fn args(convention: vun_target::Convention) -> &'static [vun_asm_arm64::Reg] {
        let mut conventions = vun_asm_arm64::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, c)| c.registers.int_args)
    }

    fn floats(convention: vun_target::Convention) -> Vec<RegRef> {
        let mut conventions = vun_asm_arm64::conventions::CONVENTIONS.iter();
        let args =
            conventions.find(|(c, _)| *c == convention).map_or(&[][..], |(_, c)| c.registers.float_args);
        args.iter().map(|v| RegRef { class: vun_asm_arm64::Class::Fp.id(), number: v.0 }).collect()
    }

    fn float_class(_: vun_asm_arm64::Size) -> Constraint {
        Constraint('w')
    }

    fn float_name(number: u8, size: vun_asm_arm64::Size) -> Option<String> {
        Some(vun_asm_arm64::V(number).name(size).to_string())
    }

    fn name(r: vun_asm_arm64::Reg) -> String {
        r.name(vun_asm_arm64::Size::B64).to_string()
    }

    fn text(inst: &Inst<Value, Output, Named>, operands: &[vun_asm_arm64::Operand<Named>]) -> String {
        let op = vun_asm_arm64::Op::entry(inst.op, inst.form).0;
        vun_asm_arm64::Inst::new(op, vun_asm_arm64::Size::of(inst.size), operands).to_string()
    }

    fn read_stack(dst: vun_asm_arm64::Reg) -> String {
        let operands: [vun_asm_arm64::Operand<Named>; 2] = [dst.into(), vun_asm_arm64::Reg::Sp.into()];
        vun_asm_arm64::Inst::new(vun_asm_arm64::Op::Mov, vun_asm_arm64::Size::B64, &operands).to_string()
    }
}

/// A block's symbols by their names in the object, where the object format names them as C does
/// and GNU C keeps what only a block names: ELF. Elsewhere they are refused.
pub(super) struct Linked<'e, 'u> {
    pub(super) e: &'e Emitter<'u>,
}

impl Rebind<Value, Output, Address> for Linked<'_, '_> {
    type Value = Value;
    type Place = Output;
    type Symbol = Named;
    type Error = AsmOperand;

    fn value(&mut self, value: Value) -> Result<Value, AsmOperand> {
        Ok(value)
    }

    fn place(&mut self, place: Output) -> Result<Output, AsmOperand> {
        Ok(place)
    }

    fn symbol(&mut self, address: Address) -> Result<Named, AsmOperand> {
        let target = self.e.target;
        if target.c.compiler.attributes != Attributes::Gnu || !ELF.is(target.object_format) {
            return Err(AsmOperand::Symbol);
        }
        // An import through its entry in the global offset table, as the native backend reaches it.
        let via = if matches!(address, Address::Import(_)) { Via::Got } else { Via::Direct };
        Ok(Named::Symbol(Symbol::intern(&self.e.names.spell(self.e.ident(address))), via))
    }
}

/// Where a block's operands are in the C statement: its bindings in the registers given them, in
/// order, and its lines at their labels.
pub(super) struct Given {
    pub(super) regs: std::vec::IntoIter<RegRef>,
    pub(super) scope: Scope,
}

impl<I: Registered> Place<I, Named> for Given {
    fn bound(&mut self, _: ClassId) -> RegRef {
        self.regs.next().expect("a register for each binding")
    }

    fn line(&self, line: Line) -> I::Operand<Named> {
        I::target(Named::Line(Labelled { line, scope: self.scope }))
    }

    fn via(&self, symbol: Named) -> Via {
        match symbol {
            Named::Symbol(_, via) => via,
            Named::Line(_) => Via::Direct,
        }
    }
}

impl Gnu for Thumb {
    const ENTER: &'static [&'static str] = &[];
    const LEAVE: &'static [&'static str] = &[];

    const SCRATCH: &'static [vun_asm_thumb::Reg] = &[vun_asm_thumb::Reg::R12];

    fn args(convention: vun_target::Convention) -> &'static [vun_asm_thumb::Reg] {
        let mut conventions = vun_asm_thumb::conventions::CONVENTIONS.iter();
        conventions.find(|(c, _)| *c == convention).map_or(&[], |(_, c)| c.registers.int_args)
    }

    fn floats(convention: vun_target::Convention) -> Vec<RegRef> {
        let mut conventions = vun_asm_thumb::conventions::CONVENTIONS.iter();
        let args =
            conventions.find(|(c, _)| *c == convention).map_or(&[][..], |(_, c)| c.registers.float_args);
        args.iter().map(|&v| RegRef { class: vun_asm_thumb::Class::Fp.id(), number: v.number() }).collect()
    }

    /// Single-precision registers are `t`, double-precision ones `w`.
    fn float_class(size: vun_asm_thumb::Size) -> Constraint {
        if size == vun_asm_thumb::Size::B64 { Constraint('w') } else { Constraint('t') }
    }

    /// `s{n}`, or `d{n/2}` for an even `n` at 64 bits.
    fn float_name(number: u8, size: vun_asm_thumb::Size) -> Option<String> {
        let v = vun_asm_thumb::V::ALL[usize::from(number)];
        v.at(size).map(|_| v.name_at(size).to_string())
    }

    fn name(r: vun_asm_thumb::Reg) -> String {
        r.name().to_owned()
    }

    /// The text of the form the line was checked against.
    fn text(inst: &Inst<Value, Output, Named>, operands: &[vun_asm_thumb::Operand<Named>]) -> String {
        let (op, form) = vun_asm_thumb::Op::entry(inst.op, inst.form);
        vun_asm_thumb::Inst::new(op, vun_asm_thumb::Size::of(inst.size), operands).by(form).to_string()
    }

    fn read_stack(dst: vun_asm_thumb::Reg) -> String {
        let operands: [vun_asm_thumb::Operand<Named>; 2] = [dst.into(), vun_asm_thumb::Reg::Sp.into()];
        vun_asm_thumb::Inst::new(vun_asm_thumb::Op::Mov, vun_asm_thumb::Size::B32, &operands).to_string()
    }
}
