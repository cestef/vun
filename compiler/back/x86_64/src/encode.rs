//! Encoding: an instruction's bytes from the form its operands fit. Prefixes, REX, the opcode,
//! `ModRM` with SIB and displacement, then immediates and a 32-bit label or call target.

use vun_asm::{RelocKind, Untaken, Via};

use crate::inst::Inst;
use crate::operand::{Base, Mem, Operand};
use crate::reg::{Reg, Size};
use crate::table::{Form, Layout, Op, Prefix, W};
use crate::{Asm, Rel32};

const REX: u8 = 0x40;
const OPERAND_16: u8 = 0x66;

/// Emitting instructions into a function's code.
pub trait Emit<S: Copy> {
    /// Emits `op` at `size` over `operands`, by the first form they fit.
    ///
    /// # Panics
    /// Panics if no form takes them: callers choose operands a form takes (`Op::form`).
    fn emit(&mut self, op: Op, size: Size, operands: &[Operand<S>]);

    /// Emits `op` at `size` over `operands` by its form `form`, the one a block of inline
    /// assembly was checked against.
    ///
    /// # Errors
    /// `form` does not take them.
    fn emit_by(
        &mut self,
        op: Op,
        form: &'static Form,
        size: Size,
        operands: &[Operand<S>],
    ) -> Result<(), Untaken>;
}

impl<S: Copy> Emit<S> for Asm<S> {
    fn emit(&mut self, op: Op, size: Size, operands: &[Operand<S>]) {
        let Some(form) = op.form(size, operands) else {
            panic!("no form of `{}` at {size:?} takes these {} operands", op.mnemonic(), operands.len())
        };
        put(self, op, form, size, operands);
    }

    fn emit_by(
        &mut self,
        op: Op,
        form: &'static Form,
        size: Size,
        operands: &[Operand<S>],
    ) -> Result<(), Untaken> {
        if !form.takes(size, operands) {
            return Err(Untaken);
        }
        put(self, op, form, size, operands);
        Ok(())
    }
}

/// Encodes `op` by `form`, listing it where the code is listed.
fn put<S: Copy>(asm: &mut Asm<S>, op: Op, form: &Form, size: Size, operands: &[Operand<S>]) {
    let at = asm.here();
    encode(asm, form, size, operands);
    if asm.lists() {
        asm.list(at, Inst::new(op, size, operands));
    }
}

/// The three bits and the REX bit of a register operand.
fn fields<S>(operand: &Operand<S>) -> (u8, bool) {
    match operand {
        Operand::Reg(r) => (r.low(), r.high()),
        Operand::Xmm(x) => (x.low(), x.high()),
        _ => unreachable!("the form puts a register here"),
    }
}

fn encode<S: Copy>(asm: &mut Asm<S>, form: &Form, size: Size, operands: &[Operand<S>]) {
    if form.prefix == Prefix::Lock {
        asm.put(&[0xf0]);
    }
    if form.w == W::Size && size == Size::B16 {
        asm.put(&[OPERAND_16]);
    }
    if let Some(prefix) = form.prefix.byte().filter(|_| form.prefix != Prefix::Lock) {
        asm.put(&[prefix]);
    }
    let (reg, rm, plus) = match form.layout {
        Layout::RegRm { reg, rm } => {
            (Some(fields(&operands[reg as usize])), Some(&operands[rm as usize]), None)
        }
        Layout::Digit { digit, rm } => (Some((digit, false)), Some(&operands[rm as usize]), None),
        Layout::PlusReg(i) => (None, None, Some(fields(&operands[i as usize]))),
        Layout::Bare => (None, None, None),
    };
    let w = match form.w {
        W::Size | W::Rex => size == Size::B64,
        W::Always => true,
        W::Never => false,
    };
    let base_high = match rm {
        Some(Operand::Mem(Mem { base: Base::Reg(r), .. })) => r.high(),
        Some(o @ (Operand::Reg(_) | Operand::Xmm(_))) => fields(o).1,
        _ => plus.is_some_and(|(_, high)| high),
    };
    // A byte register past the first four needs REX, even with no bit of it set.
    let byte_rex = form
        .kinds()
        .zip(operands)
        .any(|(k, o)| k.width(size) == Some(Size::B8) && matches!(o, Operand::Reg(r) if r.byte_needs_rex()));
    let rex = REX | u8::from(w) << 3 | u8::from(reg.is_some_and(|(_, high)| high)) << 2 | u8::from(base_high);
    if rex != REX || byte_rex {
        asm.put(&[rex]);
    }
    let mut opcode = [0; 3];
    let bytes = form.opcode.bytes();
    opcode[..bytes.len()].copy_from_slice(bytes);
    let last = &mut opcode[bytes.len() - 1];
    if let (Size::B8, Some(byte)) = (size, form.byte) {
        *last = byte;
    }
    if let Some(Operand::Cond(cc)) = operands.iter().find(|o| matches!(o, Operand::Cond(_))) {
        *last += *cc as u8;
    }
    if let Some((low, _)) = plus {
        *last += low;
    }
    asm.put(&opcode[..bytes.len()]);
    // What follows a RIP-relative displacement, which it is counted from the end of.
    let trailing: u32 = form.kinds().filter_map(|k| k.imm_bytes(size)).sum();
    if let (Some((field, _)), Some(rm)) = (reg, rm) {
        modrm(asm, field, rm, trailing);
    }
    for (kind, operand) in form.kinds().zip(operands) {
        match operand {
            Operand::Imm(v) if kind.is_imm() => {
                let n = kind.imm_bytes(size).expect("an immediate") as usize;
                asm.put(&v.to_le_bytes()[..n]);
            }
            Operand::Label(label) => {
                asm.refer(asm.here(), *label, Rel32);
                asm.put(&[0; 4]);
            }
            Operand::Sym(target) => {
                asm.relocate(asm.here(), *target, RelocKind::Call32, -4);
                asm.put(&[0; 4]);
            }
            _ => {}
        }
    }
}

/// `ModRM` for `field` and the register or memory `rm`, with SIB and displacement; `trailing`
/// bytes follow the displacement.
fn modrm<S: Copy>(asm: &mut Asm<S>, field: u8, rm: &Operand<S>, trailing: u32) {
    match rm {
        Operand::Reg(_) | Operand::Xmm(_) => asm.put(&[0xc0 | field << 3 | fields(rm).0]),
        Operand::Mem(Mem { base: Base::Reg(b), disp }) => {
            let (mode, wide) = match disp {
                0 if b.low() != Reg::Rbp.low() => (0b00, None),
                d if i8::try_from(*d).is_ok() => (0b01, Some(false)),
                _ => (0b10, Some(true)),
            };
            asm.put(&[mode << 6 | field << 3 | b.low()]);
            if b.low() == Reg::Rsp.low() {
                asm.put(&[0x24]);
            }
            match wide {
                Some(false) => asm.put(&disp.to_le_bytes()[..1]),
                Some(true) => asm.put(&disp.to_le_bytes()),
                None => {}
            }
        }
        Operand::Mem(Mem { base: Base::Rip(target, via), disp }) => {
            asm.put(&[field << 3 | 0b101]);
            let kind = match via {
                Via::Direct => RelocKind::Pc32,
                Via::Got => RelocKind::GotPc32,
            };
            asm.relocate(asm.here(), *target, kind, i64::from(*disp) - 4 - i64::from(trailing));
            asm.put(&[0; 4]);
        }
        _ => unreachable!("the form puts a register or memory here"),
    }
}
