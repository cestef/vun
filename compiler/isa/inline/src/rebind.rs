//! Carrying a block from one representation to the next, binding by binding.

use crate::{Base, Block, Bound, Disp, Inst, Mem, Operand};

/// How the bindings of one representation become those of the next, or why one cannot.
pub trait Rebind<V, P, S> {
    type Value;
    type Place;
    type Symbol;
    type Error;

    /// # Errors
    /// Why the value has no counterpart.
    fn value(&mut self, value: V) -> Result<Self::Value, Self::Error>;

    /// # Errors
    /// Why the place has no counterpart.
    fn place(&mut self, place: P) -> Result<Self::Place, Self::Error>;

    /// # Errors
    /// Why the symbol has no counterpart.
    fn symbol(&mut self, symbol: S) -> Result<Self::Symbol, Self::Error>;
}

/// A block rebound by `R`.
type Rebound<R, V, P, S> =
    Block<<R as Rebind<V, P, S>>::Value, <R as Rebind<V, P, S>>::Place, <R as Rebind<V, P, S>>::Symbol>;

/// An operand rebound by `R`.
type ReboundOperand<R, V, P, S> =
    Operand<<R as Rebind<V, P, S>>::Value, <R as Rebind<V, P, S>>::Place, <R as Rebind<V, P, S>>::Symbol>;

impl<V, P, S> Block<V, P, S> {
    /// The same block, each binding rebound by `to`, in order.
    ///
    /// # Errors
    /// The first error `to` gives.
    pub fn rebind<R: Rebind<V, P, S>>(self, to: &mut R) -> Result<Rebound<R, V, P, S>, R::Error> {
        let insts = self.insts.into_iter().map(|inst| {
            Ok(Inst {
                op: inst.op,
                form: inst.form,
                size: inst.size,
                operands: inst.operands.into_iter().map(|o| o.rebind(to)).collect::<Result<_, _>>()?,
                origin: inst.origin,
            })
        });
        Ok(Block {
            arch: self.arch,
            insts: insts.collect::<Result<_, _>>()?,
            returns: self.returns,
            needs: self.needs,
        })
    }
}

impl<V, P, S> Operand<V, P, S> {
    fn rebind<R: Rebind<V, P, S>>(self, to: &mut R) -> Result<ReboundOperand<R, V, P, S>, R::Error> {
        Ok(match self {
            Self::Reg(r) => Operand::Reg(r),
            Self::Imm(n) => Operand::Imm(n),
            Self::Cond(c) => Operand::Cond(c),
            Self::Choice(k) => Operand::Choice(k),
            Self::Label(l) => Operand::Label(l),
            Self::List(regs) => Operand::List(regs),
            Self::Symbol(s) => Operand::Symbol(to.symbol(s)?),
            Self::Bound(b) => Operand::Bound(b.rebind(to)?),
            Self::Mem(m) => Operand::Mem(Mem {
                base: m.base.map(|b| b.rebind(to)).transpose()?,
                index: m.index.map(|(b, scale)| Ok((b.rebind(to)?, scale))).transpose()?,
                disp: match m.disp {
                    Disp::Bytes(n) => Disp::Bytes(n),
                    Disp::Symbol(s, n) => Disp::Symbol(to.symbol(s)?, n),
                },
            }),
        })
    }
}

impl<V, P> Base<V, P> {
    fn rebind<S, R: Rebind<V, P, S>>(self, to: &mut R) -> Result<Base<R::Value, R::Place>, R::Error> {
        Ok(match self {
            Self::Reg(r) => Base::Reg(r),
            Self::Bound(b) => Base::Bound(b.rebind(to)?),
        })
    }
}

impl<V, P> Bound<V, P> {
    fn rebind<S, R: Rebind<V, P, S>>(self, to: &mut R) -> Result<Bound<R::Value, R::Place>, R::Error> {
        Ok(match self {
            Self::In { value, reg } => Bound::In { value: to.value(value)?, reg },
            Self::Out { place, reg } => Bound::Out { place: to.place(place)?, reg },
            Self::InOut { place, reg } => Bound::InOut { place: to.place(place)?, reg },
        })
    }
}
