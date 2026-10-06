//! Blocks of inline assembly of `lang.arch.wasm32`: each instruction is the form its table
//! declares, with its operands, so a block is the code the program wrote. An input binds the local
//! holding its value; an output is a local of the block, stored to its place after the last
//! instruction, and an input and output (`inout`) is loaded from it before the first. A place
//! the block names again is the same local, which an instruction may read after another wrote it.

use vun_backend::Missing;
use vun_inline::{Bound, Operand};
use vun_isa::Blocks;
use vun_isa_wasm32::{Choice, Global, Kind, Op as Table, Wasm32};
use vun_lir::func::Def;
use vun_lir::{Asm, InstKind, Output, Slot, Value};

use super::abi::val;
use super::func::Lower;
use crate::module::asm::{Asm as Instruction, Operand as Local};
use crate::module::op::Op;

impl Lower<'_> {
    /// Emits the block `block`.
    ///
    /// # Errors
    /// The block is of another instruction set, or binds a value the backend holds in no local.
    pub(super) fn asm(&mut self, block: &Asm) -> Result<(), Missing> {
        if block.arch != Wasm32::ARCH {
            return Err(Missing::Asm);
        }
        // The local of each place, in order: where it was loaded from or is stored to.
        let mut places: Vec<Placed> = Vec::new();
        let mut instructions = Vec::new();
        for inst in &block.insts {
            let form = Table::entry(inst.op, inst.form).1;
            let mut operands = Vec::new();
            for (at, operand) in inst.operands.iter().enumerate() {
                let arg = form.args.get(at).or(form.args.last()).ok_or(Missing::Asm)?;
                operands.push(match operand {
                    Operand::Bound(Bound::In { value, .. }) => Local::Local(self.of[*value]),
                    Operand::Bound(Bound::Out { place, .. }) => {
                        Local::Local(self.place(*place, false, &mut places)?)
                    }
                    Operand::Bound(Bound::InOut { place, .. }) => {
                        Local::Local(self.place(*place, true, &mut places)?)
                    }
                    Operand::Imm(n) if matches!(arg.kind, Kind::Imm(_) | Kind::Depths) => Local::Imm(*n),
                    Operand::Choice(at) => match arg.kind {
                        Kind::Choice(Choice::Yields) => Local::Choice(*at),
                        Kind::Choice(Choice::Global) => match Global::ALL[usize::from(*at)] {
                            Global::StackPointer => Local::Symbol(self.cx.stack_pointer),
                        },
                        _ => return Err(Missing::Asm),
                    },
                    Operand::Symbol(address) if arg.kind == Kind::Symbol => {
                        Local::Symbol(self.cx.symbol(*address))
                    }
                    _ => return Err(Missing::Asm),
                });
            }
            instructions.push(Instruction { op: inst.op, form: inst.form, operands });
        }
        for asm in instructions {
            self.op(Op::Asm(asm));
        }
        for Placed { local, place, .. } in places {
            self.get(place.addr);
            self.op(Op::LocalGet(local));
            self.op(Op::Store(Self::mem(place.ty)?));
        }
        Ok(())
    }

    /// The local standing for the place `place` in the block, a new one after the first mention,
    /// holding what the place holds where `loaded`.
    fn place(&mut self, place: Output, loaded: bool, places: &mut Vec<Placed>) -> Result<u32, Missing> {
        let key = self.key(place);
        if let Some(known) = places.iter_mut().find(|p| p.key == key && p.place.ty == place.ty) {
            if loaded && !known.loaded {
                known.loaded = true;
                let local = known.local;
                self.load_into(place, local)?;
            }
            return Ok(known.local);
        }
        let local = self.temp(val(place.ty)?);
        if loaded {
            self.load_into(place, local)?;
        }
        places.push(Placed { local, place, key, loaded });
        Ok(local)
    }

    fn load_into(&mut self, place: Output, local: u32) -> Result<(), Missing> {
        self.get(place.addr);
        self.op(Op::Load(Self::mem(place.ty)?));
        self.op(Op::LocalSet(local));
        Ok(())
    }

    /// What identifies a place: the slot a frame's address is of, as each mention of a variable
    /// computes that address again.
    fn key(&self, place: Output) -> Key {
        match self.f.values[place.addr].def {
            Def::Inst(inst) => match self.f.insts[inst].kind {
                InstKind::SlotAddr(slot) => Key::Slot(slot),
                _ => Key::Value(place.addr),
            },
            Def::Param(..) => Key::Value(place.addr),
        }
    }
}

/// What makes two places of a block the same.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Slot(Slot),
    Value(Value),
}

/// A place a block names: its local, whether the local holds what the place does, and what the
/// block stores back.
struct Placed {
    local: u32,
    place: Output,
    key: Key,
    loaded: bool,
}
