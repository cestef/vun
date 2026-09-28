//! Encoding: an instruction's halfwords from the form its operands fit, and the references to
//! labels and relocations its fields leave for later.

use vun_asm::{Label, RelocKind, Untaken};
use vun_isa_thumb::{Branch, Fixup, Form, Halves, Inst, Kind, Op, Operand, PC_AHEAD, Size, bytes};

use crate::Asm;

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
    fn emit_by(&mut self, op: Op, form: &'static Form, size: Size, operands: &[Operand<S>])
    -> Result<(), Untaken>;
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
        match Link::of(arg.kind, operand) {
            Some(Link::Label(label, fixup)) => asm.refer(at, label, fixup),
            Some(Link::Reloc(target, kind, addend)) => asm.relocate(at, target, kind, addend),
            None => {}
        }
    }
    asm.put(&bytes(word, form.wide()));
    if asm.lists() {
        asm.list(at, Inst::new(op, size, operands));
    }
}

/// The relocation filling a branch's field with a symbol's address.
const fn reloc(branch: Branch) -> Option<RelocKind> {
    match branch {
        Branch::Jump => Some(RelocKind::ThumbJump24),
        Branch::Call => Some(RelocKind::ThumbCall),
        Branch::Cond => Some(RelocKind::ThumbJump19),
        Branch::Narrow | Branch::NarrowCond | Branch::Compare => None,
    }
}

/// A field written after the instruction: when its function ends, or by the linker, which adds
/// the addend.
enum Link<S> {
    Label(Label, Fixup),
    Reloc(S, RelocKind, i64),
}

impl<S: Copy> Link<S> {
    /// What fills the field of `operand` at this position, when it is not known yet.
    fn of(kind: Kind, operand: &Operand<S>) -> Option<Self> {
        Some(match (kind, operand) {
            (Kind::Target(branch), Operand::Label(label)) => Link::Label(*label, branch.fixup()),
            // A branch counts from its program counter, ahead of its field.
            (Kind::Target(branch), Operand::Sym(target)) => Link::Reloc(*target, reloc(branch)?, -PC_AHEAD),
            (Kind::Low, Operand::Low(target)) => Link::Reloc(*target, RelocKind::ThumbLow16, 0),
            (Kind::High, Operand::High(target)) => Link::Reloc(*target, RelocKind::ThumbHigh16, 0),
            _ => return None,
        })
    }
}
