//! The verifier of LIR (spec 13, section 4): every entity has an origin, blocks, slots, values
//! and callees exist, values are defined where their uses see them (dominance), and operands
//! have the types their instructions need.

mod bounds;
mod types;

use std::collections::HashSet;
use std::fmt;

use vun_codec::{Decode, Encode};
use vun_index::{Idx, IndexVec};
use vun_origin::{OriginId, OriginTable};

use crate::analysis::{Cfg, Dominators};
use crate::entity::{Block, FuncId, Inst, Value};
use crate::func::{Def, Function};
use crate::inst::{Address, Callee, InstKind, Target};
use crate::ty::Scalar;
use crate::unit::{ImportKind, Unit};

/// Where in a unit something is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Entity {
    Func(FuncId),
    Block(FuncId, Block),
    Inst(FuncId, Inst),
    Data(usize),
    Import(usize),
}

/// Why; see [`Unit::verify`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Reason {
    NoOrigin,
    /// A function without blocks.
    Empty,
    /// The entry block's parameters are not the signature's values.
    EntryParams,
    UnknownBlock(Block),
    UnknownValue(Value),
    /// A value whose definition does not dominate a use of it.
    NotDominated(Value),
    /// An instruction in no block, or in two.
    Placement(Inst),
    /// An address or callee naming nothing in the unit.
    UnknownAddress(Address),
    /// A jump passing values that are not the target's parameters in number or type.
    Arguments(Block),
    /// Two jumps of one terminator to a block, with different arguments: moves on an edge belong
    /// to its pair of blocks (spec 13, section 8.1).
    Edge(Block),
    /// An operand, or the value defined, of a type the instruction does not accept.
    Type {
        expected: Scalar,
        found: Scalar,
    },
    /// An operation on a type it does not apply to, such as `fadd` of integers.
    Operation(Scalar),
    /// A constant with bits past its type.
    Constant,
    /// A call whose arguments are not the callee's values in number.
    Arity,
    /// A return whose value does not match the signature's result.
    Return,
    /// A signature passing an integer without how it extends, or anything else with it.
    Passing,
    /// An offset leaving the object its base points into, `at` bytes into an object of `size`.
    OutOfBounds {
        size: u64,
        at: i128,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Malformed {
    pub at: Entity,
    pub reason: Reason,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Func(id) => write!(f, "{id}"),
            Self::Block(id, block) => write!(f, "{block} of {id}"),
            Self::Inst(id, inst) => write!(f, "instruction {} of {id}", inst.index()),
            Self::Data(i) => write!(f, "d{i}"),
            Self::Import(i) => write!(f, "i{i}"),
        }
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.at, self.reason)
    }
}

impl std::error::Error for Malformed {}

impl Unit {
    /// Checks the whole unit against the invariants of the module.
    ///
    /// # Errors
    /// Returns the first violation found.
    pub fn verify(&self, origins: &OriginTable) -> Result<(), Malformed> {
        let has = |at: Entity, origin: OriginId| {
            origins.contains(origin).then_some(()).ok_or(Malformed { at, reason: Reason::NoOrigin })
        };
        for (i, import) in self.imports.iter() {
            has(Entity::Import(i.index()), import.origin)?;
            if let ImportKind::Func(signature) = &import.kind
                && !signature.well_formed()
            {
                return Err(Malformed { at: Entity::Import(i.index()), reason: Reason::Passing });
            }
        }
        for (i, data) in self.data.iter() {
            has(Entity::Data(i.index()), data.origin)?;
            for reloc in &data.relocs {
                self.address(Entity::Data(i.index()), reloc.target)?;
            }
        }
        for (id, func) in self.functions.iter() {
            Checker { unit: self, id, func, origins, homes: func.homes() }.run()?;
        }
        Ok(())
    }

    fn address(&self, at: Entity, address: Address) -> Result<(), Malformed> {
        let exists = match address {
            Address::Func(f) => self.functions.get(f).is_some(),
            Address::Data(d) => self.data.get(d).is_some(),
            Address::Import(i) => self.imports.get(i).is_some(),
        };
        exists.then_some(()).ok_or(Malformed { at, reason: Reason::UnknownAddress(address) })
    }
}

/// The checks of one function.
struct Checker<'u> {
    unit: &'u Unit,
    id: FuncId,
    func: &'u Function,
    origins: &'u OriginTable,
    /// The block holding each instruction, read once placement is checked.
    homes: IndexVec<Inst, Option<Block>>,
}

impl Checker<'_> {
    fn fail<T>(at: Entity, reason: Reason) -> Result<T, Malformed> {
        Err(Malformed { at, reason })
    }

    fn run(&self) -> Result<(), Malformed> {
        let (id, func) = (self.id, self.func);
        self.origin(Entity::Func(id), func.origin)?;
        if !func.signature.well_formed() {
            return Self::fail(Entity::Func(id), Reason::Passing);
        }
        let Some(entry) = func.entry() else { return Self::fail(Entity::Func(id), Reason::Empty) };
        let params: Vec<Scalar> = func.blocks[entry].params.iter().map(|&v| func.ty(v)).collect();
        if params != func.signature.values() {
            return Self::fail(Entity::Block(id, entry), Reason::EntryParams);
        }
        self.placement()?;
        let dominators = Dominators::of(func, &Cfg::of(func));
        for (block, data) in func.blocks.iter() {
            self.origin(Entity::Block(id, block), data.origin)?;
            for (position, &inst) in data.insts.iter().enumerate() {
                let at = Entity::Inst(id, inst);
                self.origin(at, func.insts[inst].origin)?;
                for operand in func.insts[inst].kind.operands() {
                    self.sees(at, &dominators, operand, block, Some(position))?;
                }
                self.inst(inst)?;
            }
            let at = Entity::Block(id, block);
            for operand in data.term.operands() {
                self.sees(at, &dominators, operand, block, None)?;
            }
            for (i, target) in data.term.targets().enumerate() {
                self.target(at, target)?;
                if data.term.targets().take(i).any(|t| t.block == target.block && t.args != target.args) {
                    return Self::fail(at, Reason::Edge(target.block));
                }
            }
            self.term(block)?;
        }
        self.bounds()
    }

    fn origin(&self, at: Entity, origin: OriginId) -> Result<(), Malformed> {
        if self.origins.contains(origin) { Ok(()) } else { Self::fail(at, Reason::NoOrigin) }
    }

    /// Every instruction is in exactly one block, and its value, if any, is defined by it.
    fn placement(&self) -> Result<(), Malformed> {
        let mut seen = HashSet::new();
        for (_, data) in self.func.blocks.iter() {
            for &inst in &data.insts {
                if !seen.insert(inst) {
                    return Self::fail(Entity::Inst(self.id, inst), Reason::Placement(inst));
                }
            }
        }
        match self.func.insts.iter().find(|(inst, _)| !seen.contains(inst)) {
            Some((inst, _)) => Self::fail(Entity::Inst(self.id, inst), Reason::Placement(inst)),
            None => Ok(()),
        }
    }

    /// Whether `value` is defined where a use in `block`, before the instruction at `position`
    /// (or at its terminator), sees it.
    fn sees(
        &self,
        at: Entity,
        dominators: &Dominators,
        value: Value,
        block: Block,
        position: Option<usize>,
    ) -> Result<(), Malformed> {
        let Some(data) = self.func.values.get(value) else {
            return Self::fail(at, Reason::UnknownValue(value));
        };
        let seen = match data.def {
            Def::Param(def, _) => dominators.dominates(def, block),
            Def::Inst(inst) => {
                let def = self.homes.get(inst).copied().flatten();
                match def {
                    Some(def) if def == block => {
                        let index = self.func.blocks[block].insts.iter().position(|&i| i == inst);
                        index.zip(Some(position)).is_some_and(|(i, p)| p.is_none_or(|p| i < p))
                    }
                    Some(def) => dominators.dominates(def, block),
                    None => false,
                }
            }
        };
        if seen { Ok(()) } else { Self::fail(at, Reason::NotDominated(value)) }
    }

    fn target(&self, at: Entity, target: &Target) -> Result<(), Malformed> {
        let Some(data) = self.func.blocks.get(target.block) else {
            return Self::fail(at, Reason::UnknownBlock(target.block));
        };
        let expected = data.params.iter().map(|&p| self.func.ty(p));
        let found = target.args.iter().map(|&a| self.func.ty(a));
        if expected.eq(found) { Ok(()) } else { Self::fail(at, Reason::Arguments(target.block)) }
    }

    /// The values a call passes, by its callee.
    fn callee_values(&self, at: Entity, callee: &Callee) -> Result<(Vec<Scalar>, Option<Scalar>), Malformed> {
        let signature = match callee {
            Callee::Func(f) => {
                self.unit.address(at, Address::Func(*f))?;
                &self.unit.functions[*f].signature
            }
            Callee::Import(i) => {
                self.unit.address(at, Address::Import(*i))?;
                match &self.unit.imports[*i].kind {
                    crate::unit::ImportKind::Func(signature) => signature,
                    crate::unit::ImportKind::Data => {
                        return Self::fail(at, Reason::UnknownAddress(Address::Import(*i)));
                    }
                }
            }
            Callee::Indirect(_, signature) => signature,
        };
        Ok((signature.values(), signature.returns()))
    }

    fn call(&self, inst: Inst) -> Result<(), Malformed> {
        let at = Entity::Inst(self.id, inst);
        let InstKind::Call { callee, args } = &self.func.insts[inst].kind else { return Ok(()) };
        let (params, returns) = self.callee_values(at, callee)?;
        if params.len() != args.len() {
            return Self::fail(at, Reason::Arity);
        }
        for (&expected, &arg) in params.iter().zip(args) {
            self.expect(at, arg, expected)?;
        }
        let result = self.func.insts[inst].result.map(|v| self.func.ty(v));
        if result == returns { Ok(()) } else { Self::fail(at, Reason::Return) }
    }
}
