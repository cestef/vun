//! What instructions and terminators read, and where they jump: read, and rewritten in place.
//! Each shape is written once, for shared and mutable references alike, by binding modes.

use vun_inline::Read;

use super::{Address, Callee, InstKind, Target, Terminator};
use crate::entity::Value;

/// The values an instruction reads: up to three of its own, then a call's arguments, then what
/// an assembly block binds.
macro_rules! inst_operands {
    ($inst:expr) => {
        match $inst {
            InstKind::Const { .. } | InstKind::SlotAddr(_) | InstKind::Addr(_) => {
                ([None, None, None], None, None)
            }
            InstKind::Binary { a, b, .. } | InstKind::Compare { a, b, .. } => {
                ([Some(a), Some(b), None], None, None)
            }
            InstKind::Cast { value, .. } => ([Some(value), None, None], None, None),
            InstKind::Select { cond, then, otherwise } => {
                ([Some(cond), Some(then), Some(otherwise)], None, None)
            }
            InstKind::Load { addr, .. } => ([Some(addr), None, None], None, None),
            InstKind::Store { value, addr, .. } | InstKind::Rmw { addr, value, .. } => {
                ([Some(value), Some(addr), None], None, None)
            }
            InstKind::CompareExchange { addr, expected, new, .. } => {
                ([Some(addr), Some(expected), Some(new)], None, None)
            }
            InstKind::Offset { base, offset } => ([Some(base), Some(offset), None], None, None),
            InstKind::Copy { to, from, .. } => ([Some(to), Some(from), None], None, None),
            InstKind::Call { callee: Callee::Indirect(f, _), args } => {
                ([Some(f), None, None], Some(args), None)
            }
            InstKind::Call { args, .. } => ([None, None, None], Some(args), None),
            InstKind::Asm(asm, _) => ([None, None, None], None, Some(asm)),
        }
    };
}

/// The jumps a terminator may take: a switch's cases, then the rest.
macro_rules! term_targets {
    ($term:expr) => {
        match $term {
            Terminator::Jump(t) => (None, [Some(t), None]),
            Terminator::Branch { then, otherwise, .. } => (None, [Some(then), Some(otherwise)]),
            Terminator::Switch { cases, default, .. } => (Some(cases), [Some(default), None]),
            Terminator::Return(_) | Terminator::Unreachable => (None, [None, None]),
        }
    };
}

impl Terminator {
    /// The jumps this may take: a switch's cases, then its default.
    pub fn targets(&self) -> impl Iterator<Item = &Target> {
        let (cases, rest) = term_targets!(self);
        cases.into_iter().flatten().map(|(_, t)| t).chain(rest.into_iter().flatten())
    }

    pub fn targets_mut(&mut self) -> impl Iterator<Item = &mut Target> {
        let (cases, rest) = term_targets!(self);
        cases.into_iter().flatten().map(|(_, t)| t).chain(rest.into_iter().flatten())
    }

    /// The values this reads, jump arguments included.
    pub fn operands(&self) -> impl Iterator<Item = Value> {
        let own = match self {
            Self::Branch { cond, .. } => Some(*cond),
            Self::Switch { value, .. } => Some(*value),
            Self::Return(value) => *value,
            Self::Jump(_) | Self::Unreachable => None,
        };
        own.into_iter().chain(self.targets().flat_map(|t| t.args.iter().copied()))
    }

    /// Replaces each value this reads, jump arguments included, by what `f` gives for it.
    pub fn map_values(&mut self, mut f: impl FnMut(Value) -> Value) {
        let own = match self {
            Self::Branch { cond, .. } => Some(cond),
            Self::Switch { value, .. } => Some(value),
            Self::Return(value) => value.as_mut(),
            Self::Jump(_) | Self::Unreachable => None,
        };
        if let Some(v) = own {
            *v = f(*v);
        }
        for v in self.targets_mut().flat_map(|t| t.args.iter_mut()) {
            *v = f(*v);
        }
    }
}

impl InstKind {
    /// The values this reads.
    pub fn operands(&self) -> impl Iterator<Item = Value> {
        let (own, args, asm) = inst_operands!(self);
        let bound = asm.into_iter().flat_map(|asm| asm.bindings().map(vun_inline::Bound::bound));
        own.into_iter().flatten().chain(args.into_iter().flatten()).chain(bound).copied()
    }

    /// The functions and data this names: what it takes the address of or calls, and the
    /// symbols of an assembly block and of its checks.
    pub fn addresses(&self) -> impl Iterator<Item = Address> {
        let (own, check) = match self {
            Self::Addr(a) => (Some(*a), None),
            Self::Call { callee: Callee::Func(f), .. } => (Some(Address::Func(*f)), None),
            Self::Call { callee: Callee::Import(i), .. } => (Some(Address::Import(*i)), None),
            Self::Asm(_, check) => (None, check.as_ref()),
            _ => (None, None),
        };
        own.into_iter().chain(self.symbols()).chain(check.into_iter().flat_map(|c| [c.write, c.stack]))
    }

    /// The symbols an assembly block's text names.
    pub fn symbols(&self) -> impl Iterator<Item = Address> {
        let asm = match self {
            Self::Asm(asm, _) => Some(asm),
            _ => None,
        };
        asm.into_iter().flat_map(|asm| asm.reads()).filter_map(|read| match read {
            Read::Symbol(&a) => Some(a),
            Read::Value(_) => None,
        })
    }

    /// Replaces each function, datum and import this names by what `f` gives for it: what it
    /// takes the address of or calls, and the symbols of an assembly block and of its checks. A
    /// callee stays a function or an import.
    ///
    /// # Panics
    /// If `f` maps a callee to a datum.
    pub fn map_addresses(&mut self, mut f: impl FnMut(Address) -> Address) {
        match self {
            Self::Addr(a) => *a = f(*a),
            Self::Call { callee, .. } => {
                let to = match *callee {
                    Callee::Func(g) => f(Address::Func(g)),
                    Callee::Import(i) => f(Address::Import(i)),
                    Callee::Indirect(..) => return,
                };
                *callee = to.callee().expect("a callee maps to what can be called");
            }
            Self::Asm(asm, check) => {
                for read in asm.reads_mut() {
                    if let Read::Symbol(symbol) = read {
                        *symbol = f(*symbol);
                    }
                }
                if let Some(check) = check {
                    check.write = f(check.write);
                    check.stack = f(check.stack);
                }
            }
            _ => {}
        }
    }

    /// Replaces each value this reads by what `f` gives for it.
    pub fn map_values(&mut self, mut f: impl FnMut(Value) -> Value) {
        let (own, args, asm) = inst_operands!(self);
        let bound = asm.into_iter().flat_map(|asm| asm.bindings_mut().map(vun_inline::Bound::bound_mut));
        for v in own.into_iter().flatten().chain(args.into_iter().flatten()).chain(bound) {
            *v = f(*v);
        }
    }
}
