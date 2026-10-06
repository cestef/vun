//! How blocks end.

use cranelift_codegen::ir::{BlockArg, InstBuilder, TrapCode};
use cranelift_frontend::Switch;
use vun_lir::func::BlockData;
use vun_lir::{Target, Terminator};

use super::Lower;

/// Where control never gets (`Terminator::Unreachable`).
const UNREACHABLE: TrapCode = TrapCode::unwrap_user(1);

impl Lower<'_, '_> {
    pub(super) fn terminate(&mut self, block: &BlockData) {
        self.locate(block.origin);
        match &block.term {
            Terminator::Jump(target) => {
                let (to, args) = self.jump_to(target);
                self.b.ins().jump(to, &args);
            }
            Terminator::Branch { cond, then, otherwise } => {
                let (then, then_args) = self.jump_to(then);
                let (otherwise, otherwise_args) = self.jump_to(otherwise);
                let cond = self.get(*cond);
                self.b.ins().brif(cond, then, &then_args, otherwise, &otherwise_args);
            }
            Terminator::Switch { value, cases, default } => {
                let mut switch = Switch::new();
                let mut trampolines = Vec::new();
                for (case, target) in cases {
                    let made = self.entered(target, &mut trampolines);
                    switch.set_entry(*case, made);
                }
                let otherwise = self.entered(default, &mut trampolines);
                let value = self.get(*value);
                switch.emit(&mut self.b, value, otherwise);
                for (made, target) in trampolines {
                    self.b.switch_to_block(made);
                    let (to, args) = self.jump_to(target);
                    self.b.ins().jump(to, &args);
                }
            }
            Terminator::Return(value) => {
                let values = match self.returning {
                    Some((at, parts)) => self.load_parts(parts, at),
                    None => value.iter().map(|&v| self.get(v)).collect(),
                };
                self.b.ins().return_(&values);
            }
            Terminator::Unreachable => {
                self.b.ins().trap(UNREACHABLE);
            }
        }
    }

    /// The block `target` jumps to and the values it passes.
    fn jump_to(&self, target: &Target) -> (cranelift_codegen::ir::Block, Vec<BlockArg>) {
        let args = target.args.iter().map(|&v| BlockArg::Value(self.get(v))).collect();
        (self.blocks[target.block].expect("a jump goes to a reached block"), args)
    }

    /// A block a switch enters for `target`: its own where it takes no values, else one passing
    /// them on, which is filled once the switch is emitted.
    fn entered<'t>(
        &mut self,
        target: &'t Target,
        trampolines: &mut Vec<(cranelift_codegen::ir::Block, &'t Target)>,
    ) -> cranelift_codegen::ir::Block {
        if target.args.is_empty() {
            return self.blocks[target.block].expect("a jump goes to a reached block");
        }
        let made = self.b.create_block();
        trampolines.push((made, target));
        made
    }
}
