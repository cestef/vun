//! The memory operations of `lang.mem` at run time: cells, addresses and pointers, the parts of
//! a slice, reads, writes, copies and fills, volatile accesses and atomics. Each runs whether
//! its value is kept or not: an access is an effect.

use vun_lir::{Access, BinOp, CastOp, Cond, InstKind, Order, RmwOp, Scalar, Value};
use vun_mir::Operand;
use vun_sema::primitives::mem::Mem;
use vun_sema::ty::{Ty, Value as Constant};

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

/// The orders of `lang.mem.Order`, as LIR's.
const ORDERS: [(vun_items::Item, Order); 5] = [
    (vun_items::lang::mem_Order::relaxed, Order::Relaxed),
    (vun_items::lang::mem_Order::acquire, Order::Acquire),
    (vun_items::lang::mem_Order::release, Order::Release),
    (vun_items::lang::mem_Order::acq_rel, Order::AcqRel),
    (vun_items::lang::mem_Order::seq_cst, Order::SeqCst),
];

impl FnLower<'_, '_, '_> {
    /// The primitives of `lang.mem`, `runtime` their run-time arguments, the value into `dest`
    /// when it has a place.
    pub(super) fn memory(
        &mut self,
        m: Mem,
        runtime: &[&Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        let word = self.l.types.word;
        let value = match m {
            // A cell holds its value in place.
            Mem::cell => {
                let vun_sema::ty::TyKind::Primitive(_, args) = &*ty else { return Err(unsupported()) };
                let inner = args.first().and_then(Constant::as_type).ok_or_else(unsupported)?;
                return self.write(arg(0)?, dest, inner);
            }
            // An address as another kind of address: a borrow's or a cell's as a pointer, or a
            // pointer to another type.
            Mem::pointer_to | Mem::cell_pointer | Mem::cast => self.pointed(arg(0)?)?,
            part @ (Mem::len | Mem::items_pointer) => {
                let from = self.operand_addr(arg(0)?)?;
                let (scalar, offset) = match part {
                    Mem::len => (word, word.bytes(self.l.types.pointer_bits())),
                    _ => (Scalar::Ptr, 0),
                };
                let at = self.at_offset(from, offset);
                self.b.load(scalar, at, Access::Plain, self.at)
            }
            Mem::items_at => {
                let (addr, len) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                let Some(dest) = dest else { return Ok(()) };
                self.b.store(addr, dest, Access::Plain, self.at);
                let at = self.at_offset(dest, word.bytes(self.l.types.pointer_bits()));
                self.b.store(len, at, Access::Plain, self.at);
                return Ok(());
            }
            Mem::address => {
                let addr = self.scalar_operand(arg(0)?)?;
                self.b.cast(CastOp::PtrToInt, addr, word, self.at)
            }
            // `n` items of the pointee past the address, `n` signed.
            Mem::offset => {
                let stride = self.stride(ty.pointee().ok_or_else(unsupported)?)?;
                let (addr, n) = (self.scalar_operand(arg(0)?)?, self.int_operand(arg(1)?, word)?);
                self.scaled(addr, n, stride)
            }
            Mem::read => {
                let from = self.scalar_operand(arg(0)?)?;
                return match dest {
                    Some(dest) => self.copy_value(from, dest, ty),
                    None => Ok(()),
                };
            }
            load @ (Mem::volatile_load | Mem::atomic_load) => {
                let access = match load {
                    Mem::volatile_load => Access::Volatile,
                    _ => Access::Atomic(self.order(arg(1)?)?),
                };
                let from = self.scalar_operand(arg(0)?)?;
                let scalar = self.scalar_of(ty)?;
                self.b.load(scalar, from, access, self.at)
            }
            rmw @ (Mem::atomic_swap | Mem::atomic_add | Mem::atomic_sub) => {
                let op = match rmw {
                    Mem::atomic_swap => RmwOp::Swap,
                    Mem::atomic_add => RmwOp::Add,
                    _ => RmwOp::Sub,
                };
                let (addr, value) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                let order = self.order(arg(2)?)?;
                let scalar = self.scalar_of(ty)?;
                let kind = InstKind::Rmw { op, addr, value, order };
                self.b.push(kind, Some(scalar), self.at).expect("an exchange gives what was held")
            }
            Mem::atomic_compare_swap => {
                let (expected, new) = (self.scalar_operand(arg(1)?)?, self.scalar_operand(arg(2)?)?);
                let order = self.order(arg(3)?)?;
                let failure = match order {
                    Order::AcqRel | Order::Acquire => Order::Acquire,
                    Order::SeqCst => Order::SeqCst,
                    Order::Relaxed | Order::Release => Order::Relaxed,
                };
                let addr = self.scalar_operand(arg(0)?)?;
                let kind = InstKind::CompareExchange { addr, expected, new, success: order, failure };
                let scalar = self.scalar_of(ty)?;
                self.b.push(kind, Some(scalar), self.at).expect("an exchange gives what was held")
            }
            _ => return self.effect(m, runtime),
        };
        self.keep(value, dest);
        Ok(())
    }

    /// The primitives of `lang.mem` run for what they do, with no value: writes, copies, fills.
    fn effect(&mut self, m: Mem, runtime: &[&Operand]) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        let word = self.l.types.word;
        match m {
            Mem::write => {
                let to = self.scalar_operand(arg(0)?)?;
                let ty = self.operand_ty(arg(1)?).ok_or_else(unsupported)?;
                self.write(arg(1)?, Some(to), ty)
            }
            store @ (Mem::volatile_store | Mem::atomic_store) => {
                let access = match store {
                    Mem::volatile_store => Access::Volatile,
                    _ => Access::Atomic(self.order(arg(2)?)?),
                };
                let (to, value) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                self.b.store(value, to, access, self.at);
                Ok(())
            }
            // Item by item, backward when the items copied to start past those copied from, so
            // that overlapping runs copy as if through a buffer. Each item goes through a
            // temporary, since it may overlap the one it is copied to by less than a stride.
            Mem::copy => {
                let ty = self.operand_ty(arg(0)?).and_then(|t| t.pointee()).ok_or_else(unsupported)?;
                let stride = self.stride(ty)?;
                let (to, from) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                let n = self.int_operand(arg(2)?, word)?;
                let (at_to, at_from) = (
                    self.b.cast(CastOp::PtrToInt, to, word, self.at),
                    self.b.cast(CastOp::PtrToInt, from, word, self.at),
                );
                let backward = self.b.compare(Cond::UGt, at_to, at_from, self.at);
                let Some(staged) = self.temp(ty)? else { return Ok(()) };
                self.each(n, |this, i| {
                    let one = this.b.constant(word, 1, this.at);
                    let last = this.b.binary(BinOp::Sub, n, one, this.at);
                    let reversed = this.b.binary(BinOp::Sub, last, i, this.at);
                    let i = this.b.select(backward, reversed, i, this.at);
                    let (dest, src) = (this.scaled(to, i, stride), this.scaled(from, i, stride));
                    this.copy_value(src, staged, ty)?;
                    this.copy_value(staged, dest, ty)
                })
            }
            Mem::fill => {
                let ty = self.operand_ty(arg(1)?).ok_or_else(unsupported)?;
                let stride = self.stride(ty)?;
                let to = self.scalar_operand(arg(0)?)?;
                let n = self.int_operand(arg(2)?, word)?;
                let value = self.temp(ty)?;
                self.write(arg(1)?, value, ty)?;
                self.each(n, |this, i| {
                    let dest = this.scaled(to, i, stride);
                    match value {
                        Some(value) => this.copy_value(value, dest, ty),
                        None => Ok(()),
                    }
                })
            }
            _ => Err(unsupported()),
        }
    }

    /// How many bytes apart items of type `ty` are.
    fn stride(&self, ty: Ty) -> Result<u64, Unlowered> {
        Ok(self.l.types.shape(ty, self.at)?.layout.size)
    }

    /// The memory ordering `operand` names; one known only at run time is the strongest, which
    /// every other allows.
    fn order(&self, operand: &Operand) -> Result<Order, Unlowered> {
        let Operand::Const(Constant::Variant { name, .. }) = operand else { return Ok(Order::SeqCst) };
        ORDERS
            .iter()
            .find(|(item, _)| item.symbol() == *name)
            .map(|&(_, order)| order)
            .ok_or(Unlowered::unsupported(self.at, Form::Primitive))
    }
}
