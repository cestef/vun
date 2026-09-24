//! Primitives called at run time: integer arithmetic in the storage of the result's range
//! (spec 04, rule 1), comparisons of integers through their borrows, narrowings, and the memory
//! operations of `lang.mem`.

use vun_desugar::Narrowing;
use vun_lir::{Access, BinOp, Cond, InstKind, Order, Scalar, Value};
use vun_mir::Operand;
use vun_sema::Primitive;
use vun_sema::callable::Callable;
use vun_sema::primitives::int::Int;
use vun_sema::primitives::mem::Mem;
use vun_sema::ty::{Ty, Value as Constant};

use super::FnLower;
use crate::unlowered::{Form, Unlowered};

/// The orderings of `lang.mem.Ordering`, as LIR's.
const ORDERS: [(vun_items::Item, Order); 5] = [
    (vun_items::lang::mem_Ordering::relaxed, Order::Relaxed),
    (vun_items::lang::mem_Ordering::acquire, Order::Acquire),
    (vun_items::lang::mem_Ordering::release, Order::Release),
    (vun_items::lang::mem_Ordering::acq_rel, Order::AcqRel),
    (vun_items::lang::mem_Ordering::seq_cst, Order::SeqCst),
];

impl FnLower<'_, '_, '_> {
    pub(crate) fn primitive(
        &mut self,
        p: Primitive,
        args: &[Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let comptime = Callable::of(Constant::Primitive(p))
            .and_then(|c| c.comptime(&self.l.env))
            .ok_or_else(unsupported)?;
        let runtime: Vec<&Operand> =
            args.iter().zip(&comptime).filter(|&(_, &c)| !c).map(|(a, _)| a).collect();
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        let binary = |op| match p {
            Primitive::Int(_) => Some(op),
            _ => None,
        };
        let op = match p {
            Primitive::Int(Int::add) => binary(BinOp::Add),
            Primitive::Int(Int::sub) => binary(BinOp::Sub),
            Primitive::Int(Int::mul) => binary(BinOp::Mul),
            Primitive::Int(Int::bit_and) => binary(BinOp::And),
            Primitive::Int(Int::bit_or) => binary(BinOp::Or),
            Primitive::Int(Int::bit_xor) => binary(BinOp::Xor),
            Primitive::Int(Int::shl) => binary(BinOp::Shl),
            Primitive::Int(Int::shr) => binary(BinOp::LShr),
            _ => None,
        };
        if let Some(op) = op {
            return self.arithmetic(op, arg(0)?, arg(1)?, dest, ty);
        }
        let cond = match p {
            Primitive::Int(Int::equal) => Some(Cond::Eq),
            Primitive::Int(Int::unequal) => Some(Cond::Ne),
            Primitive::Int(Int::less) => Some(Cond::SLt),
            Primitive::Int(Int::less_equal) => Some(Cond::SLe),
            Primitive::Int(Int::greater) => Some(Cond::SGt),
            Primitive::Int(Int::greater_equal) => Some(Cond::SGe),
            _ => None,
        };
        if let Some(cond) = cond {
            let (x, y) = (self.through(arg(0)?)?, self.through(arg(1)?)?);
            let holds = self.b.compare(cond, x, y, self.at);
            return self.condition(holds, dest, ty);
        }
        let Some(dest) = dest else { return Ok(()) };
        match p {
            Primitive::Int(narrowing @ (Int::to | Int::wrap | Int::sat)) => {
                let narrowing = match narrowing {
                    Int::wrap => Narrowing::Wrap,
                    Int::sat => Narrowing::Sat,
                    _ => Narrowing::To,
                };
                self.narrow(arg(0)?, narrowing, Some(dest), ty)
            }
            Primitive::Int(Int::bit_not) => {
                let (int, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
                let hi = ty
                    .interval()
                    .and_then(|r| r.hi().finite().and_then(vun_sema::int::Integer::to_u128))
                    .ok_or_else(unsupported)?;
                let x = self.int_operand(arg(0)?, int)?;
                let mask = self.b.constant(int, hi, self.at);
                let value = self.b.binary(BinOp::Xor, x, mask, self.at);
                self.b.store(value, dest, Access::Plain, self.at);
                Ok(())
            }
            Primitive::Int(Int::neg) => {
                let (int, _) = self.l.types.int(ty).ok_or_else(unsupported)?;
                let x = self.int_operand(arg(0)?, int)?;
                let zero = self.b.constant(int, 0, self.at);
                let value = self.b.binary(BinOp::Sub, zero, x, self.at);
                self.b.store(value, dest, Access::Plain, self.at);
                Ok(())
            }
            Primitive::Mem(m) => self.memory(m, &runtime, dest, ty),
            _ => Err(unsupported()),
        }
    }

    /// The primitives of `lang.mem`, `runtime` their run-time arguments.
    fn memory(&mut self, m: Mem, runtime: &[&Operand], dest: Value, ty: Ty) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        match m {
            // A cell holds its value in place.
            Mem::cell => {
                let vun_sema::ty::TyKind::Primitive(_, args) = &*ty else { return Err(unsupported()) };
                let inner = args.first().and_then(Constant::as_type).ok_or_else(unsupported)?;
                self.write(arg(0)?, Some(dest), inner)
            }
            // An address as another kind of address: a borrow's or a cell's as a pointer.
            Mem::pointer_to | Mem::cell_pointer => {
                let addr = self.scalar_operand(arg(0)?)?;
                self.b.store(addr, dest, Access::Plain, self.at);
                Ok(())
            }
            part @ (Mem::len | Mem::items_pointer) => {
                let from = self.operand_addr(arg(0)?)?;
                let (scalar, offset) = match part {
                    Mem::len => (self.l.types.word(), self.l.types.word().bytes(self.l.types.pointer_bits())),
                    _ => (Scalar::Ptr, 0),
                };
                let at = self.at_offset(from, offset);
                let value = self.b.load(scalar, at, Access::Plain, self.at);
                self.b.store(value, dest, Access::Plain, self.at);
                Ok(())
            }
            Mem::items_at => {
                let (addr, len) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                self.b.store(addr, dest, Access::Plain, self.at);
                let at = self.at_offset(dest, self.l.types.word().bytes(self.l.types.pointer_bits()));
                self.b.store(len, at, Access::Plain, self.at);
                Ok(())
            }
            Mem::read => {
                let from = self.scalar_operand(arg(0)?)?;
                self.copy_value(from, dest, ty)
            }
            Mem::atomic_load => {
                let order = self.order(arg(1)?)?;
                let from = self.scalar_operand(arg(0)?)?;
                let scalar = self.scalar_of(ty)?;
                let value = self.b.load(scalar, from, Access::Atomic(order), self.at);
                self.b.store(value, dest, Access::Plain, self.at);
                Ok(())
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
                let held = self.b.push(kind, Some(scalar), self.at).expect("an exchange gives what was held");
                self.b.store(held, dest, Access::Plain, self.at);
                Ok(())
            }
            _ => self.effect(Primitive::Mem(m), runtime),
        }
    }

    /// Primitives run for what they do, with no value: writes.
    fn effect(&mut self, p: Primitive, runtime: &[&Operand]) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        match p {
            Primitive::Mem(Mem::write) => {
                let to = self.scalar_operand(arg(0)?)?;
                let ty = self.operand_ty(arg(1)?).ok_or_else(unsupported)?;
                self.write(arg(1)?, Some(to), ty)
            }
            Primitive::Mem(Mem::atomic_store) => {
                let order = self.order(arg(2)?)?;
                let (to, value) = (self.scalar_operand(arg(0)?)?, self.scalar_operand(arg(1)?)?);
                self.b.store(value, to, Access::Atomic(order), self.at);
                Ok(())
            }
            _ => Err(unsupported()),
        }
    }

    /// `x op y` in the storage of the result's range `ty`.
    fn arithmetic(
        &mut self,
        op: BinOp,
        x: &Operand,
        y: &Operand,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let (int, signed) = self.l.types.int(ty).ok_or(Unlowered::unsupported(self.at, Form::Primitive))?;
        let (x, y) = (self.int_operand(x, int)?, self.int_operand(y, int)?);
        let op = if op == BinOp::LShr && signed { BinOp::AShr } else { op };
        let value = self.b.binary(op, x, y, self.at);
        self.b.store(value, dest, Access::Plain, self.at);
        Ok(())
    }

    /// The integer `operand` in the storage `int`.
    pub(crate) fn int_operand(&mut self, operand: &Operand, int: Scalar) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let ty = self.operand_ty(operand).ok_or_else(unsupported)?;
        let (own, signed) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let value = self.scalar_operand(operand)?;
        Ok(self.resize(value, own, signed, int))
    }

    /// The integer a borrow `operand` reaches, as a signed 128-bit integer, so any two compare.
    fn through(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let ty = self.operand_ty(operand).and_then(|t| t.pointee()).ok_or_else(unsupported)?;
        let (int, signed) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let addr = self.scalar_operand(operand)?;
        let value = self.b.load(int, addr, Access::Plain, self.at);
        Ok(self.resize(value, int, signed, Scalar::I128))
    }

    /// The scalar `operand` holds.
    pub(crate) fn scalar_operand(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let ty = self.operand_ty(operand).ok_or(Unlowered::unsupported(self.at, Form::Primitive))?;
        match operand {
            Operand::Const(value) => {
                let scalar = self.scalar_of(ty)?;
                self.scalar_const(value, ty, scalar)
            }
            Operand::Place(_) => {
                let addr = self.operand_addr(operand)?;
                self.load_scalar(addr, ty)
            }
        }
    }

    /// The address holding `operand`: its place's, or a temporary's holding the constant.
    pub(crate) fn operand_addr(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let ty = self.operand_ty(operand).ok_or_else(unsupported)?;
        match operand {
            Operand::Place(place) => self.reach(place)?.map(|r| r.addr).ok_or_else(unsupported),
            Operand::Const(value) => {
                let temp = self.temp(ty)?.ok_or_else(unsupported)?;
                self.write_const(value, temp, ty)?;
                Ok(temp)
            }
        }
    }

    fn scalar_of(&self, ty: Ty) -> Result<Scalar, Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            crate::repr::Repr::Scalar(s) => Ok(s),
            _ => Err(Unlowered::unsupported(self.at, Form::Primitive)),
        }
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
