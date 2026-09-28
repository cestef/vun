//! Calls: the plan holds what is used after the call in registers it preserves or in homes, and
//! the arguments passed in general registers in them when the call is reached (`Cx::call_needs`);
//! the rest goes where the callee's convention passes it from wherever the plan holds it,
//! records the convention passes by address as the address of a copy in the caller's frame, and
//! a result comes back in registers or through memory.

use vun_asm_thumb::{Emit, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::Arch;
use vun_lir::{AbiType, Address, Callee, Ext, Signature, Value};
use vun_native::width::offset_of;

use super::Sel;
use crate::abi::{Carrier, Loc, Passing, Ret};
use crate::asm::AsmExt;
use crate::{FAR, FP, Instr, Thumb2};

impl Sel<'_, '_> {
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let f = self.f;
        let signature: &Signature = self.signature(callee);
        let passing = Passing::of(signature, self.state.convention).map_err(|u| self.lack(u.into()))?;
        self.frame.outgoing(passing.stack);
        // What goes through memory first, through the framework's registers: arguments on the
        // stack, records copied there, and the copies of records passed by address.
        let mut copies = Vec::new();
        for (&a, loc) in args.iter().zip(&passing.params) {
            match loc {
                Loc::Stack { offset, bytes } => self.outgoing(a, *offset, *bytes),
                Loc::Copied { offset, size } => self.copy_out(a, 0, *offset, *size),
                Loc::Split { rest: Some(rest), .. } => self.copy_out(a, rest.from, rest.offset, rest.bytes),
                Loc::Indirect { size, at } => {
                    let copy = self.frame.reserve(*size, Thumb2::STACK_ALIGN);
                    self.reg_to(a, Thumb2::CARRIER);
                    self.asm.offset(Thumb2::TEMP, FP, i64::from(copy));
                    self.copy_bytes(Thumb2::TEMP, Thumb2::CARRIER, *size, FAR);
                    if let Loc::Stack { offset, bytes } = **at {
                        self.store_outgoing(Thumb2::TEMP, offset, bytes);
                    }
                    copies.push(copy);
                }
                Loc::Reg(_) | Loc::Split { rest: None, .. } | Loc::Returned => {}
            }
        }
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
                    // An address known while compiling is written there by the call itself; a
                    // value kept is there already, the plan having put it.
                    (_, None) => {
                        if self.recomputed(a).is_some() {
                            self.reg_to(a, *r);
                        }
                    }
                },
                Loc::Split { parts, .. } => {
                    self.reg_to(a, Thumb2::CARRIER);
                    for part in parts {
                        self.load_part(
                            part.reg,
                            Thumb2::CARRIER,
                            i64::from(offset_of(part.offset)),
                            part.bytes,
                        );
                    }
                }
                Loc::Indirect { at, .. } => {
                    let copy = copies.next().expect("copied above");
                    if let Loc::Reg(Carrier::Int(r)) = **at {
                        self.asm.offset(r, FP, i64::from(copy));
                    }
                }
                Loc::Stack { .. } | Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        match callee {
            Callee::Func(f) => self.asm.emit(Op::Bl, Size::B32, &[Operand::Sym(Address::Func(*f))]),
            Callee::Import(i) => self.asm.emit(Op::Bl, Size::B32, &[Operand::Sym(Address::Import(*i))]),
            Callee::Indirect(target, _) => {
                self.reg_to(*target, Thumb2::CARRIER);
                self.asm.emit(Op::Blx, Size::B32, &[Thumb2::CARRIER.into()]);
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
                self.reg_to(out, Thumb2::CARRIER);
                for part in parts {
                    self.store_part(Thumb2::CARRIER, i64::from(offset_of(part.offset)), part.reg, part.bytes);
                }
            }
            // The plan takes a result in a general register from there.
            Ret::Reg(Carrier::Int(_)) | Ret::None | Ret::Memory => {}
        }
        Ok(())
    }

    /// Copies the `bytes` from `from` of the record at `a` to `offset` of the outgoing arguments.
    fn copy_out(&mut self, a: Value, from: u64, offset: u64, bytes: u64) {
        self.reg_to(a, Thumb2::CARRIER);
        if from > 0 {
            self.asm.offset(Thumb2::CARRIER, Thumb2::CARRIER, i64::from(offset_of(from)));
        }
        self.asm.offset(Thumb2::TEMP, Reg::Sp, i64::from(offset_of(offset)));
        self.copy_bytes(Thumb2::TEMP, Thumb2::CARRIER, bytes, FAR);
    }

    /// Writes `a` at `offset` of the outgoing arguments.
    fn outgoing(&mut self, a: Value, offset: u64, bytes: u64) {
        self.reg_to(a, Thumb2::CARRIER);
        self.store_outgoing(Thumb2::CARRIER, offset, bytes);
    }

    /// Stores the `bytes` low bytes of `r` where the stack argument at `offset` goes.
    fn store_outgoing(&mut self, r: Reg, offset: u64, bytes: u64) {
        let size =
            <Size as vun_asm::Width>::of_bytes(bytes).expect("stack arguments are scalars or addresses");
        let (op, size) = Instr::Store.at(size);
        self.asm.access(op, size, r.into(), Reg::Sp, i64::from(offset_of(offset)), FAR);
    }

    /// Extends an integer of `size` in `r` to 32 bits, as the C convention expects of callers
    /// and callees.
    pub(super) fn extend(&mut self, r: Reg, size: Size, ext: Ext) {
        if size.narrow() {
            let extend = match ext {
                Ext::Zero => Instr::Zext,
                Ext::Sign => Instr::Sext,
            };
            let (op, size) = extend.at(size);
            self.asm.emit(op, size, &[r.into(), r.into()]);
        }
    }
}
