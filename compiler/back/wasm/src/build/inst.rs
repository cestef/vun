//! Instructions: each reads its operands from their locals and writes its value to its own. An
//! integer narrower than 32 bits is held zero-extended, so an operation reading it signed
//! (`Reads::Signed`) extends it first, and every result of one is masked.

use vun_backend::{Lack, Missing};
use vun_lir::{Address, BinOp, CallKind, CastOp, Cond, Inst, InstKind, Reads, RmwOp, Scalar, Value};
use vun_target::Threads;

use super::abi::{narrow, val};
use super::func::Lower;
use super::select;
use crate::module::ValType;
use crate::module::op::{Num, Op};

/// Dispatches up to this many table entries per case with a `br_table`, more with compares.
const DENSITY: u128 = 4;

impl Lower<'_> {
    pub(super) fn inst(&mut self, inst: Inst) -> Result<(), Lack> {
        let data = &self.f.insts[inst];
        let at = data.origin;
        let result = data.result;
        self.mark(at);
        let start = self.body.len();
        let emitted = self.emit(data.kind.clone(), result, at);
        vun_trace::tracing::trace!(inst = %data.kind, ops = self.body.len() - start, "select");
        emitted
    }

    fn emit(&mut self, kind: InstKind, result: Option<Value>, at: vun_origin::OriginId) -> Result<(), Lack> {
        match kind {
            InstKind::Const { ty, bits } => self.constant(ty, bits).map_err(|m| m.at(at))?,
            InstKind::Binary { op, a, b } => self.binary(op, a, b).map_err(|m| m.at(at))?,
            InstKind::Compare { cond, a, b } => self.compare(cond, a, b).map_err(|m| m.at(at))?,
            InstKind::Cast { op, value, to } => self.cast(op, value, to).map_err(|m| m.at(at))?,
            InstKind::Select { cond, then, otherwise } => {
                self.get(then);
                self.get(otherwise);
                self.get(cond);
                self.op(Op::Select);
            }
            InstKind::Load { ty, addr, access: _ } => {
                self.get(addr);
                self.op(Op::Load(Self::mem(ty).map_err(|m| m.at(at))?));
            }
            InstKind::Store { value, addr, access: _ } => {
                self.get(addr);
                self.get(value);
                self.op(Op::Store(Self::mem(self.f.ty(value)).map_err(|m| m.at(at))?));
            }
            InstKind::Rmw { op, addr, value, order: _ } => {
                let r = result.expect("a read-modify-write gives what memory held");
                self.rmw(op, addr, value, r).map_err(|m| m.at(at))?;
                return Ok(());
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                let r = result.expect("a compare-exchange gives what memory held");
                self.exchange(addr, expected, new, r).map_err(|m| m.at(at))?;
                return Ok(());
            }
            InstKind::Offset { base, offset } => {
                self.get(base);
                self.get(offset);
                self.op(Op::Num(Num::I32Add));
            }
            InstKind::SlotAddr(slot) => self.slot_addr(slot),
            InstKind::Addr(address) => self.op(Op::Address(self.cx.symbol(address), 0)),
            InstKind::ThreadAddr(data) => match self.cx.threads {
                Threads::One => self.op(Op::Address(self.cx.symbol(Address::Data(data)), 0)),
                Threads::Many => return Err(Missing::PerThread.at(at)),
            },
            InstKind::Copy { to, from, bytes } => {
                self.get(to);
                self.get(from);
                self.op(Op::I32Const(Self::i32(bytes)));
                self.op(Op::MemoryCopy);
            }
            InstKind::Call { kind: CallKind::Tail, .. } => return Err(Missing::TailCall.at(at)),
            InstKind::Call { callee, args, .. } => return self.call(&callee, &args, result, at),
            InstKind::Asm(block, check) => return self.asm(&block, check.as_ref()).map_err(|m| m.at(at)),
        }
        if let Some(r) = result {
            self.set(r);
        }
        Ok(())
    }

    fn constant(&mut self, ty: Scalar, bits: u128) -> Result<(), Missing> {
        #[expect(clippy::cast_possible_truncation, reason = "a constant's bits fit its type")]
        self.op(match val(ty)? {
            ValType::I32 => Op::I32Const((bits as u32).cast_signed()),
            ValType::I64 => Op::I64Const((bits as u64).cast_signed()),
            ValType::F32 => Op::F32Const(bits as u32),
            ValType::F64 => Op::F64Const(bits as u64),
            ValType::V128 => return Err(Missing::Vector),
        });
        Ok(())
    }

    /// Pushes `v` as `reads` reads it: a narrow integer read signed is extended.
    fn read(&mut self, v: Value, reads: Reads) {
        self.get(v);
        if let (Reads::Signed, Some(bits)) = (reads, narrow(self.f.ty(v))) {
            self.extend(bits);
        }
    }

    fn binary(&mut self, op: BinOp, a: Value, b: Value) -> Result<(), Missing> {
        let ty = self.f.ty(a);
        self.read(a, op.reads());
        self.read(b, op.reads());
        self.op(Op::Num(select::binary(op, val(ty)?)));
        if let Some(bits) = narrow(ty) {
            self.mask(bits);
        }
        Ok(())
    }

    fn compare(&mut self, cond: Cond, a: Value, b: Value) -> Result<(), Missing> {
        let ty = val(self.f.ty(a))?;
        self.read(a, cond.reads());
        self.read(b, cond.reads());
        if cond == Cond::FNe {
            // Ordered: false where either is not a number, which `ne` is not.
            let (lt, gt) =
                if ty == ValType::F32 { (Num::F32Lt, Num::F32Gt) } else { (Num::F64Lt, Num::F64Gt) };
            let (x, y) = (self.temp(ty), self.temp(ty));
            self.op(Op::LocalSet(y));
            self.op(Op::LocalTee(x));
            self.op(Op::LocalGet(y));
            self.op(Op::Num(lt));
            self.op(Op::LocalGet(x));
            self.op(Op::LocalGet(y));
            self.op(Op::Num(gt));
            self.op(Op::Num(Num::I32Or));
            return Ok(());
        }
        self.op(Op::Num(select::compare(cond, ty)));
        Ok(())
    }

    fn cast(&mut self, op: CastOp, v: Value, to: Scalar) -> Result<(), Missing> {
        let from = self.f.ty(v);
        self.read(v, op.reads());
        let (a, b) = (val(from)?, val(to)?);
        let num = if a == b {
            // Between values of one type the bits stay; a narrow result is masked below.
            None
        } else {
            Some(select::cast(op, a, b))
        };
        if let Some(num) = num {
            self.op(Op::Num(num));
        }
        if let Some(bits) = narrow(to) {
            self.mask(bits);
        }
        Ok(())
    }

    /// `op` of what `addr` holds and `value`, written back; the value is what it held.
    fn rmw(&mut self, op: RmwOp, addr: Value, value: Value, result: Value) -> Result<(), Missing> {
        let ty = self.f.ty(value);
        let mem = Self::mem(ty)?;
        self.get(addr);
        self.get(addr);
        self.op(Op::Load(mem));
        self.op(Op::LocalTee(self.of[result]));
        let bin = match op {
            RmwOp::Swap => None,
            RmwOp::Add => Some(BinOp::Add),
            RmwOp::Sub => Some(BinOp::Sub),
            RmwOp::And => Some(BinOp::And),
            RmwOp::Or => Some(BinOp::Or),
            RmwOp::Xor => Some(BinOp::Xor),
        };
        if let Some(bin) = bin {
            self.get(value);
            self.op(Op::Num(select::binary(bin, val(ty)?)));
        } else {
            self.op(Op::Drop);
            self.get(value);
        }
        self.op(Op::Store(mem));
        Ok(())
    }

    /// Writes `new` at `addr` where it holds `expected`; the value is what it held.
    fn exchange(&mut self, addr: Value, expected: Value, new: Value, result: Value) -> Result<(), Missing> {
        let ty = self.f.ty(new);
        let mem = Self::mem(ty)?;
        self.get(addr);
        self.op(Op::Load(mem));
        self.set(result);
        self.get(addr);
        self.get(new);
        self.get(result);
        self.get(result);
        self.get(expected);
        self.op(Op::Num(select::compare(Cond::Eq, val(ty)?)));
        self.op(Op::Select);
        self.op(Op::Store(mem));
        Ok(())
    }

    /// Branches to the landing of the case equal to `value`, or of `default`: through a
    /// `br_table` over the cases' range when they are dense, by compares otherwise.
    pub(super) fn dispatch(&mut self, value: Value, cases: &[(u128, u32)], default: u32) {
        let ty = val(self.f.ty(value)).expect("a switched value is an integer");
        let top = cases.iter().map(|&(n, _)| n).max().unwrap_or(0);
        if top < (cases.len() as u128 + 1) * DENSITY {
            let count = u32::try_from(top + 1).expect("dense tables are small");
            self.get(value);
            if ty == ValType::I64 {
                self.op(Op::I64Const(i64::from(count)));
                self.op(Op::Num(Num::I64GeU));
                self.op(Op::BrIf(default));
                self.get(value);
                self.op(Op::Num(Num::I32WrapI64));
            }
            let table = (0..u128::from(count))
                .map(|n| cases.iter().find(|&&(c, _)| c == n).map_or(default, |&(_, l)| l))
                .collect();
            self.op(Op::BrTable { cases: table, default });
            return;
        }
        for &(n, landing) in cases {
            self.get(value);
            self.constant(self.f.ty(value), n).expect("a switched value is an integer");
            self.op(Op::Num(select::compare(Cond::Eq, ty)));
            self.op(Op::BrIf(landing));
        }
        self.op(Op::Br(default));
    }
}
