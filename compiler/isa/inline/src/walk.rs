//! What a block binds and reads, in operand order, read and rewritten in place. Each shape is
//! written once, for shared and mutable references alike, by binding modes.

use crate::{Base, Block, Bound, Disp, Mem, Operand};

/// What a block reads: a value bound, or a symbol.
pub enum Read<V, S> {
    Value(V),
    Symbol(S),
}

impl<T> Read<T, T> {
    /// What is read, where values and symbols are of one kind.
    pub fn read(self) -> T {
        match self {
            Self::Value(t) | Self::Symbol(t) => t,
        }
    }
}

/// The bindings of an operand, itself or its address's registers, and the symbol it names.
macro_rules! parts {
    ($operand:expr) => {
        match $operand {
            Operand::Bound(b) => ([Some(b), None], None),
            Operand::Symbol(s) => ([None, None], Some(s)),
            Operand::Mem(Mem { base, index, disp }) => {
                let base = match base {
                    Some(Base::Bound(b)) => Some(b),
                    _ => None,
                };
                let index = match index {
                    Some((Base::Bound(b), _)) => Some(b),
                    _ => None,
                };
                let symbol = match disp {
                    Disp::Symbol(s, _) => Some(s),
                    Disp::Bytes(_) => None,
                };
                ([base, index], symbol)
            }
            Operand::Reg(_)
            | Operand::Imm(_)
            | Operand::Cond(_)
            | Operand::Choice(_)
            | Operand::Label(_)
            | Operand::List(_) => ([None, None], None),
        }
    };
}

impl<V, P, S> Block<V, P, S> {
    /// Every value bound, in order.
    pub fn bindings(&self) -> impl Iterator<Item = &Bound<V, P>> {
        self.insts.iter().flat_map(|inst| &inst.operands).flat_map(|o| parts!(o).0.into_iter().flatten())
    }

    pub fn bindings_mut(&mut self) -> impl Iterator<Item = &mut Bound<V, P>> {
        self.insts
            .iter_mut()
            .flat_map(|inst| &mut inst.operands)
            .flat_map(|o| parts!(o).0.into_iter().flatten())
    }

    /// The places the block writes, `{out p}` and `{inout p}`, in order, each with whether the
    /// block reads it first.
    pub fn outputs(&self) -> impl Iterator<Item = (&P, bool)> {
        self.bindings().filter_map(|b| match b {
            Bound::Out { place, .. } => Some((place, false)),
            Bound::InOut { place, .. } => Some((place, true)),
            Bound::In { .. } => None,
        })
    }

    /// The values, `{in e}`, and the symbols the block reads, in order.
    pub fn reads(&self) -> impl Iterator<Item = Read<&V, &S>> {
        self.insts.iter().flat_map(|inst| &inst.operands).flat_map(|o| {
            let (bindings, symbol) = parts!(o);
            let values = bindings.into_iter().flatten().filter_map(|b| match b {
                Bound::In { value, .. } => Some(Read::Value(value)),
                Bound::Out { .. } | Bound::InOut { .. } => None,
            });
            values.chain(symbol.map(Read::Symbol))
        })
    }

    pub fn reads_mut(&mut self) -> impl Iterator<Item = Read<&mut V, &mut S>> {
        self.insts.iter_mut().flat_map(|inst| &mut inst.operands).flat_map(|o| {
            let (bindings, symbol) = parts!(o);
            let values = bindings.into_iter().flatten().filter_map(|b| match b {
                Bound::In { value, .. } => Some(Read::Value(value)),
                Bound::Out { .. } | Bound::InOut { .. } => None,
            });
            values.chain(symbol.map(Read::Symbol))
        })
    }
}

impl<V, P, S> Block<V, P, S> {
    /// Whether an instruction names a register itself, rather than binding a value to one the
    /// backend picks or one a binding names (`in(e).at(r)`): such a block reads or writes state
    /// of the function it is in.
    #[must_use]
    pub fn names_registers(&self) -> bool {
        let named = |base: &Option<Base<V, P>>| matches!(base, Some(Base::Reg(_)));
        self.insts.iter().flat_map(|inst| &inst.operands).any(|operand| match operand {
            Operand::Reg(_) | Operand::List(_) => true,
            Operand::Mem(Mem { base, index, .. }) => named(base) || matches!(index, Some((Base::Reg(_), _))),
            _ => false,
        })
    }
}

impl<V, P: AsRef<V> + AsMut<V>> Bound<V, P> {
    /// The value it binds, where a place is reached through one: an input's own, an output's
    /// address.
    pub fn bound(&self) -> &V {
        match self {
            Self::In { value, .. } => value,
            Self::Out { place, .. } | Self::InOut { place, .. } => place.as_ref(),
        }
    }

    pub fn bound_mut(&mut self) -> &mut V {
        match self {
            Self::In { value, .. } => value,
            Self::Out { place, .. } | Self::InOut { place, .. } => place.as_mut(),
        }
    }
}
