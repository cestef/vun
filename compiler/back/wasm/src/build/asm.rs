//! Blocks of inline assembly of `lang.arch.wasm32`: each instruction is the form its table
//! declares, with its operands, so a block is the code the program wrote. An input binds the local
//! holding its value; an output is a local of the block, stored to its place after the last
//! instruction, and an input and output (`inout`) is loaded from it before the first.

use vun_backend::Missing;
use vun_inline::{Bound, Operand};
use vun_isa::Blocks;
use vun_isa_wasm32::{Kind, Op as Table, Wasm32};
use vun_lir::{Asm, Output};

use super::abi::val;
use super::func::Lower;
use crate::module::op::{Asm as Instruction, Op, Operand as Local};

impl Lower<'_> {
    /// Emits the block `block`.
    ///
    /// # Errors
    /// The block is of another instruction set, or binds a value the backend holds in no local.
    pub(super) fn asm(&mut self, block: &Asm) -> Result<(), Missing> {
        if block.arch != Wasm32::ARCH {
            return Err(Missing::Asm);
        }
        // The local of each binding, in order: where it was loaded from or is stored to.
        let mut places: Vec<(u32, Output)> = Vec::new();
        let mut instructions = Vec::new();
        for inst in &block.insts {
            let form = Table::entry(inst.op, inst.form).1;
            let mut operands = Vec::new();
            for (arg, operand) in form.args.iter().zip(&inst.operands) {
                operands.push(match operand {
                    Operand::Bound(Bound::In { value, .. }) => Local::Local(self.of[*value]),
                    Operand::Bound(Bound::Out { place, .. }) => {
                        Local::Local(self.place(*place, false, &mut places)?)
                    }
                    Operand::Bound(Bound::InOut { place, .. }) => {
                        Local::Local(self.place(*place, true, &mut places)?)
                    }
                    Operand::Imm(n) if matches!(arg.kind, Kind::Imm(_)) => Local::Imm(*n),
                    _ => return Err(Missing::Asm),
                });
            }
            instructions.push(Instruction { op: inst.op, form: inst.form, operands });
        }
        for asm in instructions {
            self.op(Op::Asm(asm));
        }
        for (local, place) in places {
            self.get(place.addr);
            self.op(Op::LocalGet(local));
            self.op(Op::Store(Self::mem(place.ty)?));
        }
        Ok(())
    }

    /// The local standing for the place `place` in the block, a new one after the first mention,
    /// holding what the place holds where `loaded`.
    fn place(
        &mut self,
        place: Output,
        loaded: bool,
        places: &mut Vec<(u32, Output)>,
    ) -> Result<u32, Missing> {
        let ty = val(place.ty)?;
        let local = self.temp(ty);
        if loaded {
            self.get(place.addr);
            self.op(Op::Load(Self::mem(place.ty)?));
            self.op(Op::LocalSet(local));
        }
        places.push((local, place));
        Ok(local)
    }
}
