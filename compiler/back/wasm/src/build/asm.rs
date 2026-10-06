//! Blocks of inline assembly of `lang.arch.wasm32`: each instruction is the form its table
//! declares, with its operands, so a block is the code the program wrote. An input binds the local
//! holding its value; an output is a local of the block, stored to its place after the last
//! instruction, and an input and output (`inout`) is loaded from it before the first. A place
//! the block names again is the same local, which an instruction may read after another wrote it.
//! Under the `asm` checks, each instruction writing memory is preceded by a call of the runtime
//! with the address and the bytes it writes, and a block that moves the stack pointer and returns
//! is checked at its end.

use vun_backend::Missing;
use vun_inline::{Bound, Operand};
use vun_isa::Blocks;
use vun_isa_wasm32::{Choice, Form, Global, Kind, Op as Table, Wasm32, Written};
use vun_lir::func::Def;
use vun_lir::{Asm, AsmCheck, InstKind, Output, Slot, Value};

use super::abi::val;
use super::func::Lower;
use crate::module::ValType;
use crate::module::asm::Typed;
use crate::module::asm::{Asm as Instruction, Operand as Local};
use crate::module::op::{Num, Op};

impl Lower<'_> {
    /// Emits the block `block`.
    ///
    /// # Errors
    /// The block is of another instruction set, or binds a value the backend holds in no local.
    pub(super) fn asm(&mut self, block: &Asm, check: Option<&AsmCheck>) -> Result<(), Missing> {
        if block.arch != Wasm32::ARCH {
            return Err(Missing::Asm);
        }
        // The local of each place, in order: where it was loaded from or is stored to.
        let mut places: Vec<Placed> = Vec::new();
        let mut instructions = Vec::new();
        let moves = block.insts.iter().any(|inst| Table::entry(inst.op, inst.form).0 == Table::GlobalSet);
        let before = check.filter(|_| moves && block.returns).map(|_| self.temp(ValType::I32));
        if let Some(before) = before {
            instructions.extend([Op::GlobalGet(self.cx.stack_pointer), Op::LocalSet(before)]);
        }
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
            if let Some(check) = check {
                instructions.extend(self.guard(form, &operands, check)?);
            }
            instructions.push(Op::Asm(Instruction { op: inst.op, form: inst.form, operands }));
        }
        if let (Some(check), Some(before)) = (check, before) {
            instructions.extend(self.unmoved(before, check));
        }
        for op in instructions {
            self.op(op);
        }
        for Placed { local, place, .. } in places {
            self.get(place.addr);
            self.op(Op::LocalGet(local));
            self.op(Op::Store(Self::mem(place.ty)?));
        }
        Ok(())
    }

    /// What runs before an instruction of `form` writing memory, if it does: its operands are kept
    /// in locals, the runtime told the address and the bytes, and the operands put back.
    fn guard(&mut self, form: &Form, operands: &[Local], check: &AsmCheck) -> Result<Vec<Op>, Missing> {
        let written = match form.writes {
            Written::None => return Ok(Vec::new()),
            written => written,
        };
        let kept =
            form.pops.iter().map(|ty| ty.val_type().ok_or(Missing::Asm)).collect::<Result<Vec<_>, _>>()?;
        let locals: Vec<u32> = kept.into_iter().map(|ty| self.temp(ty)).collect();
        let (address, rest) = locals.split_first().ok_or(Missing::Asm)?;
        let mut ops: Vec<Op> = rest.iter().rev().map(|&l| Op::LocalSet(l)).collect();
        ops.extend([Op::LocalTee(*address), Op::LocalGet(*address)]);
        // The offset follows the alignment among the operands of an access.
        if let Some(Local::Imm(offset)) = operands.get(1).filter(|_| operands.len() >= 2) {
            ops.extend([
                Op::I32Const(i32::try_from(*offset).map_err(|_| Missing::Asm)?),
                Op::Num(Num::I32Add),
            ]);
        }
        ops.push(match written {
            Written::Bytes(bytes) => Op::I32Const(i32::from(bytes)),
            _ => Op::LocalGet(*rest.last().ok_or(Missing::Asm)?),
        });
        ops.extend([
            Op::I32Const(i32::try_from(check.site).map_err(|_| Missing::Asm)?),
            Op::Call(self.cx.symbol(check.write)),
        ]);
        ops.extend(rest.iter().map(|&l| Op::LocalGet(l)));
        Ok(ops)
    }

    /// What ends a block that moved the stack pointer and returns: the runtime told where it was
    /// and where it is now, if they differ.
    fn unmoved(&mut self, before: u32, check: &AsmCheck) -> Vec<Op> {
        let after = self.temp(ValType::I32);
        vec![
            Op::GlobalGet(self.cx.stack_pointer),
            Op::LocalTee(after),
            Op::LocalGet(before),
            Op::Num(Num::I32Ne),
            Op::If,
            Op::I32Const(i32::try_from(check.site).expect("sites fit i32")),
            Op::LocalGet(before),
            Op::LocalGet(after),
            Op::Call(self.cx.symbol(check.stack)),
            Op::Unreachable,
            Op::End,
        ]
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
