//! Encoding: an instruction's word from the form its operands fit, and the references to labels
//! and relocations its fields leave for later.

use vun_asm::{Label, RelocKind, Untaken, Via};

use crate::inst::Inst;
use crate::operand::{Mem, Operand};
use crate::reg::Size;
use crate::table::{Form, Kind, Op};
use crate::{Asm, Fixup};

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
    /// `form` does not encode them.
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
        let Some((form, word)) = op.select(size, operands) else {
            panic!("no form of `{}` at {size:?} takes these {} operands", op.mnemonic(), operands.len())
        };
        put(self, op, form, word, size, operands);
    }

    fn emit_by(
        &mut self,
        op: Op,
        form: &'static Form,
        size: Size,
        operands: &[Operand<S>],
    ) -> Result<(), Untaken> {
        let word = form.encode(size, operands).ok_or(Untaken)?;
        put(self, op, form, word, size, operands);
        Ok(())
    }
}

/// Puts `word`, `op` encoded by `form`, with the references its fields leave, listing it where
/// the code is listed.
fn put<S: Copy>(asm: &mut Asm<S>, op: Op, form: &Form, word: u32, size: Size, operands: &[Operand<S>]) {
    let at = asm.here();
    for (arg, operand) in form.args.iter().zip(operands) {
        match arg.kind.link(operand, size) {
            Some(Link::Label(label, fixup)) => asm.refer(at, label, fixup),
            Some(Link::Reloc(target, kind)) => asm.relocate(at, target, kind, 0),
            None => {}
        }
    }
    asm.put(&word.to_le_bytes());
    if asm.lists() {
        asm.list(at, Inst::new(op, size, operands));
    }
}

/// A field written after the instruction: when its function ends, or by the linker.
enum Link<S> {
    Label(Label, Fixup),
    Reloc(S, RelocKind),
}

impl Kind {
    /// What fills the field of `operand` at this position, when it is not known yet.
    fn link<S: Copy>(self, operand: &Operand<S>, size: Size) -> Option<Link<S>> {
        Some(match (self, operand) {
            (Self::Target(branch), Operand::Label(label)) => Link::Label(*label, branch.fixup()),
            (Self::Target(branch), Operand::Sym(target)) => Link::Reloc(*target, branch.reloc()),
            (Self::Page, Operand::Page(target, Via::Direct)) => Link::Reloc(*target, RelocKind::Page21),
            (Self::Page, Operand::Page(target, Via::Got)) => Link::Reloc(*target, RelocKind::GotPage21),
            (Self::Lo12, Operand::Lo12(target)) => Link::Reloc(*target, RelocKind::PageOffset12),
            (Self::Mem(_, _), Operand::Mem(Mem::Lo12 { target, via: Via::Got, .. })) => {
                Link::Reloc(*target, RelocKind::GotPageOffset64)
            }
            (Self::Mem(_, width), Operand::Mem(Mem::Lo12 { target, via: Via::Direct, .. })) => {
                let kind = match width.at(size) {
                    Size::B8 => RelocKind::PageOffset8,
                    Size::B16 => RelocKind::PageOffset16,
                    Size::B32 => RelocKind::PageOffset32,
                    Size::B64 => RelocKind::PageOffset64,
                };
                Link::Reloc(*target, kind)
            }
            _ => return None,
        })
    }
}
