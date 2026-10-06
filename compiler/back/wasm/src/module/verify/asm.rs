//! The instructions of a block of inline assembly, validated by what their forms declare.

use vun_isa_wasm32::{Control, Flow, Ty};

use super::Check;
use crate::module::ValType;
use crate::module::invalid::Why;
use crate::module::op::{Asm, Operand};

impl Check<'_> {
    /// An instruction of a block, popping and pushing what its form declares: the type of its
    /// local where it says so, any type where it says that; and opening, ending or branching to
    /// the blocks around it as it says.
    pub(super) fn asm(&mut self, asm: &Asm) -> Result<(), Why> {
        let form = vun_isa_wasm32::Op::entry(asm.op, asm.form).1;
        let local = asm.operands.iter().find_map(|o| match o {
            Operand::Local(l) => Some(self.local(*l)),
            Operand::Imm(_) => None,
        });
        let ty = |ty: Ty| match ty {
            Ty::I32 => Ok(Some(ValType::I32)),
            Ty::I64 => Ok(Some(ValType::I64)),
            Ty::F32 => Ok(Some(ValType::F32)),
            Ty::F64 => Ok(Some(ValType::F64)),
            Ty::Local => local.clone().unwrap_or(Err(Why::Unbalanced)).map(Some),
            Ty::Any => Ok(None),
        };
        for &popped in form.pops.iter().rev() {
            match ty(popped)? {
                Some(ty) => self.pop(ty)?,
                None => {
                    self.pop_any()?;
                }
            }
        }
        for &pushed in form.pushes {
            if let Some(ty) = ty(pushed)? {
                self.push(ty);
            }
        }
        let depth = asm.operands.iter().find_map(|o| match o {
            Operand::Imm(depth) => u32::try_from(*depth).ok(),
            Operand::Local(_) => None,
        });
        match (form.control, depth) {
            (Control::None, _) => {}
            (Control::Open, _) => self.open(false),
            (Control::OpenIf, _) => self.open(true),
            (Control::Reopen, _) => self.else_()?,
            (Control::Close, _) => self.end_block()?,
            (Control::Branch, Some(depth)) => self.jump(None, depth)?,
            (Control::BranchIf, Some(depth)) => {
                self.label(depth)?;
            }
            (Control::Branch | Control::BranchIf, None) => return Err(Why::Unbalanced),
        }
        if form.flow == Flow::Ends {
            self.unreachable();
        }
        Ok(())
    }
}
