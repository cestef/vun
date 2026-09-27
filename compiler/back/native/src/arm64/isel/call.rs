//! Calls: every value used after the call is stored home, since a call may write any register
//! values are held in but the callee-saved ones; arguments go where the callee's convention
//! passes them, those in general registers straight from where they are, records larger than 16 bytes as the address of a copy in the caller's frame, and a result
//! comes back in registers or through memory.

use vun_asm_arm64::{Emit, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::Arch;
use vun_lir::{AbiType, Address, Callee, Ext, ImportKind, Signature, Value};

use super::Sel;
use crate::arm64::abi::{Carrier, INDIRECT, Loc, Passing, Ret};
use crate::arm64::{Arm, FAR, FP, access, offset};

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
        // What goes through memory first, through the framework's registers: arguments on the
        // stack, records copied there, and the copies of records passed by address.
        let mut copies = Vec::new();
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Stack { offset, bytes } => self.outgoing(a, *offset, *bytes),
                Loc::Copied { offset: at, size } => {
                    self.reg_to(a, Arm::CARRIER);
                    offset(&mut self.asm, Arm::TEMP, Reg::Sp, disp(*at));
                    self.copy_bytes(Arm::TEMP, Arm::CARRIER, *size, FAR);
                }
                Loc::Indirect { size, at } => {
                    let copy = self.frame.reserve(*size, Arm::STACK_ALIGN);
                    self.reg_to(a, Arm::CARRIER);
                    offset(&mut self.asm, Arm::TEMP, FP, i64::from(copy));
                    self.copy_bytes(Arm::TEMP, Arm::CARRIER, *size, FAR);
                    if let Loc::Stack { offset, bytes } = **at {
                        self.store_outgoing(Arm::TEMP, offset, bytes);
                    }
                    copies.push(copy);
                }
                Loc::Reg(_) | Loc::Split(_) | Loc::Returned => {}
            }
        }
        self.pass_in_regs(&passed);
        self.clear();
        let mut copies = copies.into_iter();
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
                    self.reg_to(a, Arm::CARRIER);
                    for part in parts {
                        self.load_part(part.reg, Arm::CARRIER, disp(part.offset), part.bytes);
                    }
                }
                Loc::Indirect { at, .. } => {
                    let copy = copies.next().expect("copied above");
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        offset(&mut self.asm, r, FP, i64::from(copy));
                    }
                }
                Loc::Stack { .. } | Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        if passing.ret == Ret::Memory {
            let out = *args.last().expect("the address of the result is passed last");
            self.reg_to(out, INDIRECT);
        }
        match callee {
            Callee::Func(f) => self.asm.emit(Op::Bl, Size::B64, &[Operand::Sym(Address::Func(*f))]),
            Callee::Import(i) => self.asm.emit(Op::Bl, Size::B64, &[Operand::Sym(Address::Import(*i))]),
            Callee::Indirect(target, _) => {
                self.reg_to(*target, Arm::CARRIER);
                self.asm.emit(Op::Blr, Size::B64, &[Arm::CARRIER.into()]);
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
                self.reg_to(out, Arm::CARRIER);
                for part in parts {
                    self.store_part(Arm::CARRIER, disp(part.offset), part.reg, part.bytes);
                }
            }
            Ret::None | Ret::Memory => {}
        }
        Ok(())
    }

    /// Writes `a` at `offset` of the outgoing arguments.
    fn outgoing(&mut self, a: Value, offset: u64, bytes: u64) {
        self.reg_to(a, Arm::CARRIER);
        self.store_outgoing(Arm::CARRIER, offset, bytes);
    }

    /// Stores the `bytes` low bytes of `r` where the stack argument at `offset` goes.
    fn store_outgoing(&mut self, r: Reg, offset: u64, bytes: u64) {
        let size = Size::of_bytes(bytes).expect("stack arguments are scalars or addresses");
        access(&mut self.asm, Op::Str, size, r.into(), Reg::Sp, disp(offset), FAR);
    }

    /// Extends an integer of `size` in `r` to 32 bits, as the C convention expects of callers
    /// and callees on some systems.
    pub(super) fn extend(&mut self, r: Reg, size: Size, ext: Ext) {
        if size < Size::B32 {
            let op = match ext {
                Ext::Zero => Op::Uxt,
                Ext::Sign => Op::Sxt,
            };
            self.asm.emit(op, size, &[r.into(), r.into()]);
        }
    }
}

/// A byte count as a displacement.
fn disp(bytes: u64) -> i64 {
    i64::try_from(bytes).expect("records and arguments fit memory")
}
