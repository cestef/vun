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
        if kind == CallKind::Tail && !passing.in_registers() {
            return Err(self.lack(Missing::TailCall));
        }
        self.frame.outgoing(passing.stack);
        // Arguments on the stack and records copied there go through the carrier.
        for (&a, loc) in args.iter().zip(&passing.params) {
            match *loc {
                Loc::Stack { offset, .. } => {
                    self.reg_to(a, X86::CARRIER);
                    self.asm.emit(Op::Mov, Size::B64, &[outgoing(offset).into(), X86::CARRIER.into()]);
                }
                Loc::Copied { offset, size } => self.copy_out(a, 0, offset, size),
                Loc::Split { rest: Some(rest), .. } => self.copy_out(a, rest.from, rest.offset, rest.bytes),
                Loc::Reg(_) | Loc::Split { rest: None, .. } | Loc::Indirect { .. } | Loc::Returned => {}
            }
        }
        // What each LIR value passes, a wide integer's halves both, the address of a result none.
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
                Loc::Stack { .. } | Loc::Copied { .. } | Loc::Indirect { .. } | Loc::Returned => {}
            }
        }
        // A variadic C function reads how many SSE registers carry arguments from `al`.
        if signature.conv == Conv::C {
            self.asm.emit(Op::Mov, Size::B32, &[Reg::Rax.into(), i64::from(passing.floats).into()]);
        }
        if kind == CallKind::Tail {
            // The result is the callee's, which the `return` after it passes on from where it is.
            self.tail_jump(callee);
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

    /// Leaves the frame and jumps to `callee`, which returns to this function's caller.
    fn tail_jump(&mut self, callee: &Callee) {
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
