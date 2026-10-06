//! The instructions of a block of inline assembly, validated by what their forms declare.

use vun_isa::Nest;
use vun_isa_wasm32::{Control, Flow, Ty};

use super::Check;
use crate::module::Linker;
use crate::module::asm::{Asm, Operand, Typed};
use crate::module::invalid::Why;

impl Check<'_> {
    /// An instruction of a block, popping and pushing what its form declares: the type of its
    /// local where it says so, any type where it says that; and opening, ending or branching to
    /// the blocks around it as it says.
    pub(super) fn asm(&mut self, asm: &Asm) -> Result<(), Why> {
        let form = vun_isa_wasm32::Op::entry(asm.op, asm.form).1;
        let local = asm.operands.iter().find_map(|o| match o {
            Operand::Local(l) => Some(self.local(*l)),
            _ => None,
        });
        let ty = |ty: Ty| match ty {
            Ty::Local => local.clone().unwrap_or(Err(Why::Unbalanced)).map(Some),
            _ => Ok(ty.val_type()),
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
        let calls = matches!(form.control, Control::Call | Control::CallIndirect);
        if let Some(symbol) = asm.symbol().filter(|_| !calls) {
            self.stack_pointer(symbol)?;
        }
        let depths: Vec<u32> = asm.depths().collect();
        let yields = asm.yields().into_iter().collect::<Vec<_>>();
        let depth = depths.first().copied().ok_or(Why::Unbalanced);
        match form.control {
            Control::None => {}
            Control::Open => self.open(Nest::Plain, yields),
            Control::OpenLoop => self.open(Nest::Loop, yields),
            Control::OpenIf => self.open(Nest::If, yields),
            Control::Reopen => self.else_()?,
            Control::Close => self.end_block()?,
            Control::Branch => self.jump(None, depth?)?,
            Control::BranchIf => self.through(depth?)?,
            Control::Call => self.call_symbol(asm.symbol().ok_or(Why::Unbalanced)?)?,
            Control::CallIndirect => {
                let symbol = asm.symbol().ok_or(Why::Unbalanced)?;
                self.module.linker(Linker::Table).ok_or(Why::Symbol(symbol))?;
                self.call_symbol(symbol)?;
            }
            Control::BranchTable => {
                let (default, cases) = depths.split_last().ok_or(Why::Unbalanced)?;
                self.targets(cases, *default)?;
            }
        }
        if form.flow == Flow::Ends {
            self.unreachable();
        }
        Ok(())
    }
}
