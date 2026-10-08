//! Calls: the plan holds what is used after the call in registers it preserves or in homes, and
//! the arguments passed in general registers in them when the call is reached (`Cx::call_needs`);
//! the rest goes where the callee's convention passes it from wherever the plan holds it, and a
//! result comes back in registers or through memory.

use vun_asm_x86_64::{Emit, Mem, Op, Reg, Size};
use vun_backend::{Lack, Missing};
use vun_codegen::Arch;
use vun_dwarf::Step;
use vun_lir::{AbiType, Address, CallKind, Callee, Conv, Ext, Signature, Value};

use super::Sel;
use super::entry::CALLER_ARGS;
use crate::X86;
use crate::abi::{Carrier, Loc, Passing, Ret};
use vun_native::width::offset_of;

impl Sel<'_, '_> {
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        kind: CallKind,
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let f = self.f;
        let signature: &Signature = self.signature(callee);
        let passing = Passing::of(signature, self.state.convention).map_err(|u| self.lack(u.into()))?;
        if kind == CallKind::Tail {
            let received = f.entry().map(|entry| f.blocks[entry].params.as_slice()).unwrap_or_default();
            if !passing.reuses(&self.state.passing, args, received) {
                return Err(self.lack(Missing::TailCall));
            }
        }
        self.frame.outgoing(passing.stack).map_err(|m| self.lack(m))?;
        let copies = self.write_memory(args, &passing, kind);
        self.load_registers(signature, args, &passing, copies, kind)?;
        // A variadic C function reads how many SSE registers carry arguments from `al`.
        if signature.conv == Conv::C {
            self.asm.emit(Op::Mov, Size::B32, &[Reg::Rax.into(), i64::from(passing.floats).into()]);
        }
        if kind == CallKind::Tail {
            // The result is the callee's, which the `return` after it passes on from where it is.
            self.tail_jump(callee, passing.stack);
            return Ok(());
        }
        match callee {
            Callee::Func(f) => {
                self.asm.emit(Op::Call, Size::B64, &[vun_asm_x86_64::Operand::Sym(Address::Func(*f))]);
            }
            Callee::Import(i) => {
                self.asm.emit(Op::Call, Size::B64, &[vun_asm_x86_64::Operand::Sym(Address::Import(*i))]);
            }
            Callee::Indirect(target, _) => {
                self.reg_to(*target, X86::CARRIER);
                self.asm.emit(Op::Call, Size::B64, &[X86::CARRIER.into()]);
            }
        }
        match &passing.ret {
            Ret::Reg(Carrier::Float(x)) => {
                if let Some(out) = result {
                    self.receive_float(out, *x);
                }
            }
            Ret::Split(parts) => {
                let out = *args.last().expect("the address of the result is passed last");
                self.reg_to(out, X86::CARRIER);
                for part in parts {
                    self.store_part(Mem::at(X86::CARRIER, offset_of(part.offset)), part.reg, part.bytes);
                }
            }
            // The plan takes a result in a general register from there.
            Ret::Reg(Carrier::Int(_)) | Ret::None | Ret::Memory => {}
        }
        Ok(())
    }

    /// Writes what goes through memory first, through the carrier: arguments on the stack,
    /// records copied there, and the copies of records passed by address, whose offsets in the
    /// frame it returns.
    fn write_memory(&mut self, args: &[Value], passing: &Passing, kind: CallKind) -> Vec<i32> {
        let mut copies = Vec::new();
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Stack { offset, .. } => {
                    self.reg_to(a, X86::CARRIER);
                    self.asm.emit(Op::Mov, Size::B64, &[outgoing(*offset).into(), X86::CARRIER.into()]);
                }
                Loc::Copied { offset, size } => self.copy_out(a, 0, *offset, *size),
                Loc::Split { rest: Some(rest), .. } => self.copy_out(a, rest.from, rest.offset, rest.bytes),
                // A record the function received, passed on as it is.
                Loc::Indirect { at, .. } if kind == CallKind::Tail => {
                    if let Loc::Stack { offset, .. } = **at {
                        self.reg_to(a, X86::CARRIER);
                        self.asm.emit(Op::Mov, Size::B64, &[outgoing(offset).into(), X86::CARRIER.into()]);
                    }
                }
                Loc::Indirect { size, at } => {
                    let copy = self.frame.reserve(*size, X86::WORD);
                    self.reg_to(a, X86::CARRIER);
                    let to = Mem::at(Reg::Rbp, copy);
                    self.copy_bytes(to, Mem::at(X86::CARRIER, 0), *size, X86::TEMP);
                    if let Loc::Stack { offset, .. } = **at {
                        self.asm.emit(Op::Lea, Size::B64, &[X86::CARRIER.into(), to.into()]);
                        self.asm.emit(Op::Mov, Size::B64, &[outgoing(offset).into(), X86::CARRIER.into()]);
                    }
                    copies.push(copy);
                }
                Loc::Reg(_) | Loc::Split { rest: None, .. } | Loc::Returned => {}
            }
        }
        // What each LIR value passes, a wide integer's halves both, the address of a result none.
        copies
    }

    /// Puts each argument passed in a register there, and the addresses of the copies in `copies`.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn load_registers(
        &mut self,
        signature: &Signature,
        args: &[Value],
        passing: &Passing,
        copies: Vec<i32>,
        kind: CallKind,
    ) -> Result<(), Lack> {
        let f = self.f;
        let mut copies = copies.into_iter();
        let abis: Vec<&AbiType> = signature.passed().collect();
        for (i, (&a, loc)) in args.iter().zip(&passing.params).enumerate() {
            match loc {
                Loc::Reg(carrier @ Carrier::Float(_)) => self.pass(a, *carrier),
                Loc::Reg(Carrier::Int(r)) => match (abis.get(i).copied(), f.constant(a)) {
                    (Some(AbiType::Int(scalar, ext)), Some((_, bits))) => {
                        self.constant_to(*r, ext.widen(*scalar, bits));
                    }
                    (_, Some((_, bits))) => self.constant_to(*r, bits),
                    (Some(AbiType::Int(scalar, ext)), None) => {
                        let size = self.size_of(*scalar)?;
                        self.extend(*r, size, *ext);
                    }
                    // An address known while compiling is written there by the call itself; a
                    // value kept is there already, the plan having put it.
                    (_, None) => {
                        if self.recomputed(a).is_some() {
                            self.reg_to(a, *r);
                        }
                    }
                },
                Loc::Split { parts, .. } => {
                    self.reg_to(a, X86::CARRIER);
                    for part in parts {
                        self.load_part(part.reg, Mem::at(X86::CARRIER, offset_of(part.offset)), part.bytes);
                    }
                }
                Loc::Indirect { at, .. } if kind == CallKind::Tail => {
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        self.reg_to(a, r);
                    }
                }
                Loc::Indirect { at, .. } => {
                    let copy = copies.next().expect("copied above");
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        self.asm.emit(Op::Lea, Size::B64, &[r.into(), Mem::at(Reg::Rbp, copy).into()]);
                    }
                }
                Loc::Stack { .. } | Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        Ok(())
    }

    /// Writes the `stack` bytes of outgoing arguments over the ones this function received,
    /// leaves the frame and jumps to `callee`, which returns to this function's caller. They are
    /// staged first, since a value passed may be read from the area they go to.
    fn tail_jump(&mut self, callee: &Callee, stack: u64) {
        self.copy_bytes(Mem::at(Reg::Rbp, CALLER_ARGS), outgoing(0), stack, X86::TEMP);
        // The target waits in the carrier, which leaving the frame leaves alone.
        if let Callee::Indirect(target, _) = callee {
            self.reg_to(*target, X86::CARRIER);
        }
        self.leave();
        match callee {
            Callee::Func(f) => {
                self.asm.emit(Op::Jmp, Size::B64, &[vun_asm_x86_64::Operand::Sym(Address::Func(*f))]);
            }
            Callee::Import(i) => {
                self.asm.emit(Op::Jmp, Size::B64, &[vun_asm_x86_64::Operand::Sym(Address::Import(*i))]);
            }
            Callee::Indirect(..) => self.asm.emit(Op::Jmp, Size::B64, &[X86::CARRIER.into()]),
        }
        self.unwound(Step::Returned);
    }

    /// Extends an integer of `size` in `r` to 32 bits, as the C convention expects of callers
    /// and callees.
    pub(super) fn extend(&mut self, r: Reg, size: Size, ext: Ext) {
        if size < Size::B32 {
            let op = match ext {
                Ext::Zero => Op::Movzx,
                Ext::Sign => Op::Movsx,
            };
            self.asm.emit(op, size, &[r.into(), r.into()]);
        }
    }
}

impl Sel<'_, '_> {
    /// Copies the `bytes` from `from` of the record at `a` to `offset` of the outgoing arguments.
    fn copy_out(&mut self, a: Value, from: u64, offset: u64, bytes: u64) {
        self.reg_to(a, X86::CARRIER);
        self.copy_bytes(outgoing(offset), Mem::at(X86::CARRIER, offset_of(from)), bytes, X86::TEMP);
    }
}

/// Where the stack argument at `offset` goes: the bottom of the frame.
fn outgoing(offset: u64) -> Mem<Address> {
    Mem::at(Reg::Rsp, offset_of(offset))
}
