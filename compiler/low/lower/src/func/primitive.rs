//! Primitives called at run time: integer arithmetic in the storage of the result's range
//! (spec 04, rule 1), comparisons of integers through their borrows, narrowings, whether an
//! integer fits a type, the variant of an enum value, a vector as its lanes and back, and the
//! memory operations of `lang.mem` (`memory.rs`); a library's report refuses the program.

use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_primitives::Primitive;
use vun_primitives::diag::Diag;
use vun_primitives::hint::Hint;
use vun_primitives::int::Int;
use vun_primitives::meta::Meta;
use vun_primitives::simd::Simd;
use vun_sema::callable::Callable;
use vun_sema_ty::trap::Reason;
use vun_sema_ty::{Ty, Value as Constant};

use super::FnLower;
use crate::unlowered::{Form, Unlowered, Why};

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
        if let Primitive::Float(f) = p {
            return self.float(f, &runtime, dest, ty);
        }
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
            _ => None,
        };
        if let Some(op) = op {
            return self.arithmetic(op, arg(0)?, arg(1)?, dest, ty);
        }
        if let Primitive::Int(shift @ (Int::shl | Int::shr)) = p {
            let op = if shift == Int::shl { BinOp::Shl } else { BinOp::LShr };
            return self.shift(op, arg(0)?, arg(1)?, dest, ty);
        }
        if let Primitive::Int(quotient @ (Int::div | Int::rem)) = p {
            return self.divide(quotient == Int::div, arg(0)?, arg(1)?, dest, ty);
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
            let (x, y, signed) = self.compared(arg(0)?, arg(1)?)?;
            let cond = if signed { cond } else { cond.unsigned() };
            let holds = self.b.compare(cond, x, y, self.at);
            return self.condition(holds, dest, ty);
        }
        if p == Primitive::Int(Int::compare) {
            let (x, y, signed) = self.compared(arg(0)?, arg(1)?)?;
            let less = self.b.compare(if signed { Cond::SLt } else { Cond::ULt }, x, y, self.at);
            let equal = self.b.compare(Cond::Eq, x, y, self.at);
            return self.ordering(less, equal, dest, ty);
        }
        if let Primitive::Bounds(b) = p {
            return self.bounded(b, &runtime, dest, ty);
        }
        match p {
            Primitive::Int(i) => self.int_unary(i, args, &runtime, dest, ty),
            Primitive::Meta(Meta::variant) => {
                let [Operand::Const(of), _] = args else { return Err(unsupported()) };
                let of = of.as_type().ok_or_else(unsupported)?;
                let addr = self.pointed(arg(0)?)?;
                let index = self.variant_index(addr, of)?;
                self.keep(index, dest);
                Ok(())
            }
            Primitive::Meta(Meta::part) => {
                let [Operand::Const(of), _, Operand::Const(variant), Operand::Const(index)] = args else {
                    return Err(unsupported());
                };
                let of = of.as_type().ok_or_else(unsupported)?;
                let (variant, index) =
                    (variant.as_usize().ok_or_else(unsupported)?, index.as_usize().ok_or_else(unsupported)?);
                let addr = self.pointed(arg(0)?)?;
                let Some(dest) = dest else { return Ok(()) };
                let (holds, part) = self.part_at(addr, of, variant, index)?;
                self.optional(holds, part, dest, ty)
            }
            Primitive::Mem(m) => self.memory(m, &runtime, dest, ty),
            // A vector and the array of its lanes are held alike.
            Primitive::Simd(Simd::lanes | Simd::of) => self.write(arg(0)?, dest, ty),
            // A hint changes nothing the program does; after `unreachable`, the block MIR ends
            // there says so.
            Primitive::Hint(Hint::cold | Hint::unreachable) => Ok(()),
            // Library code reached at run time that reports an error refuses the program.
            Primitive::Diag(Diag::error) => {
                let constants: Option<Vec<Constant>> = args
                    .iter()
                    .map(|a| match a {
                        Operand::Const(value) => Some(value.clone()),
                        Operand::Place(_) => None,
                    })
                    .collect();
                let reason = constants.as_deref().and_then(Reason::reported).ok_or_else(unsupported)?;
                Err(Unlowered::new(at, Why::Reported(reason)))
            }
            _ => Err(unsupported()),
        }
    }

    /// The integer a borrow `operand` reaches, as a signed 128-bit integer, so any two compare.
    /// The integers the borrows `lhs` and `rhs` reach, in the narrowest integer storing both
    /// ranges, and whether it is signed.
    fn compared(&mut self, lhs: &Operand, rhs: &Operand) -> Result<(Value, Value, bool), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let pointee = |this: &Self, operand: &Operand| {
            this.operand_ty(operand).and_then(|t| t.pointee()).ok_or_else(unsupported)
        };
        let (left, right) = (pointee(self, lhs)?, pointee(self, rhs)?);
        let (common, signed) = self.l.types.common(left, right).ok_or_else(unsupported)?;
        Ok((self.through(lhs, common)?, self.through(rhs, common)?, signed))
    }

    fn through(&mut self, operand: &Operand, to: Scalar) -> Result<Value, Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let ty = self.operand_ty(operand).and_then(|t| t.pointee()).ok_or_else(unsupported)?;
        let (int, signed) = self.l.types.int(ty).ok_or_else(unsupported)?;
        let addr = self.pointed(operand)?;
        let value = self.b.load(int, addr, Access::Plain, self.at);
        Ok(self.resize(value, int, signed, to))
    }

    /// The address the borrow or pointer `operand` points to. A shared borrow of a compile-time
    /// value holds the value, so a constant one points to storage holding it.
    pub(crate) fn pointed(&mut self, operand: &Operand) -> Result<Value, Unlowered> {
        let is_address = self.operand_ty(operand).is_some_and(|t| t.pointee().is_some());
        match operand {
            Operand::Const(_) if !is_address => self.operand_addr(operand),
            _ => self.scalar_operand(operand),
        }
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

    /// The scalar `operand` holds as a value of `ty`, the type of the parameter it is passed to:
    /// a constant takes that type, where its own value would give a narrower one.
    pub(crate) fn scalar_operand_as(&mut self, operand: &Operand, ty: Ty) -> Result<Value, Unlowered> {
        match operand {
            Operand::Const(value) => {
                let scalar = self.scalar_of(ty)?;
                self.scalar_const(value, ty, scalar)
            }
            Operand::Place(_) => self.scalar_operand(operand),
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

    /// The scalar a block reads and writes a place of `ty` as: its own, or the 128 bits of a vector.
    pub(super) fn held_of(&self, ty: Ty) -> Result<Scalar, Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            crate::repr::Repr::Memory(layout)
                if layout.size == Scalar::V128.bytes(0) && ty.lanes().is_some() =>
            {
                Ok(Scalar::V128)
            }
            _ => self.scalar_of(ty),
        }
    }

    pub(super) fn scalar_of(&self, ty: Ty) -> Result<Scalar, Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            crate::repr::Repr::Scalar(s) => Ok(s),
            _ => Err(Unlowered::unsupported(self.at, Form::Primitive)),
        }
    }
}
