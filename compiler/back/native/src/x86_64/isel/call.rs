//! Calls: every value used after the call is stored home, since a call may write any register
//! values are held in but the callee-saved ones; arguments go where the callee's convention
//! passes them, those in general registers straight from where they are, and a result comes
//! back in registers or through memory.

use vun_asm_x86_64::{Emit, Mem, Op, Reg, Size};
use vun_backend::Lack;
use vun_codegen::Arch;
use vun_lir::{AbiType, Address, Callee, Conv, Ext, ImportKind, Signature, Value};

use super::Sel;
use crate::width::offset_of;
use crate::x86_64::X86;
use crate::x86_64::abi::{Carrier, Loc, Passing, Ret};

impl Sel<'_, '_> {
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let (unit, f) = (self.unit, self.f);
        let signature: &Signature = match callee {
            Callee::Func(f) => &unit.functions[*f].signature,
            Callee::Import(i) => match &unit.imports[*i].kind {
                ImportKind::Func(s) => s,
                ImportKind::Data => unreachable!("the verifier checks calls call functions"),
            },
            Callee::Indirect(_, s) => s,
        };
        let passing = Passing::of(signature, self.state.convention).map_err(|u| self.lack(u.into()))?;
        for &a in args {
            self.size(a)?;
        }
        // Constants are written where they go once the others are, already extended.
        let passed: Vec<_> =
            passing.in_ints(args).into_iter().filter(|&(a, _)| f.constant(a).is_none()).collect();
        self.spill_for_call(&passed);
        self.frame.outgoing(passing.stack);
        // Arguments on the stack and records copied there go through the carrier, before those
        // passed in registers.
        for (&a, loc) in args.iter().zip(&passing.params) {
            match *loc {
                Loc::Stack { offset, .. } => {
                    self.reg_to(a, X86::CARRIER);
                    self.asm.emit(Op::Mov, Size::B64, &[outgoing(offset).into(), X86::CARRIER.into()]);
                }
                Loc::Copied { offset, size } => {
                    self.reg_to(a, X86::CARRIER);
                    self.copy_bytes(outgoing(offset), Mem::at(X86::CARRIER, 0), size, X86::TEMP);
                }
                Loc::Reg(_) | Loc::Split(_) | Loc::Indirect { .. } | Loc::Returned => {}
            }
        }
        self.pass_in_regs(&passed);
        self.clear();
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
                    (_, None) => {}
                },
                Loc::Split(parts) => {
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
            Ret::Reg(carrier) => {
                if let Some(out) = result {
                    self.receive(out, *carrier);
                }
            }
            Ret::Split(parts) => {
                let out = *args.last().expect("the address of the result is passed last");
                self.reg_to(out, X86::CARRIER);
                for part in parts {
                    self.store_part(Mem::at(X86::CARRIER, offset_of(part.offset)), part.reg, part.bytes);
                }
            }
            Ret::None | Ret::Memory => {}
        }
        Ok(())
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

/// Where the stack argument at `offset` goes: the bottom of the frame.
fn outgoing(offset: u64) -> Mem<Address> {
    Mem::at(Reg::Rsp, offset_of(offset))
}
