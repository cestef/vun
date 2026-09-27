//! The verifier of MIR: every local, place and jump target a body names exists, parameters are
//! distinct locals apart from the result, no jump enters the first block, a frame ends only where
//! the body can suspend, and every entity has an origin in the table (spec 13, section 4).

use std::collections::HashSet;
use std::fmt;

use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_origin::{OriginId, OriginTable};

use crate::place::{Place, Projection};
use crate::{BlockId, Body, Local, Operand, Rvalue, TerminatorKind};

/// The entities of a body, for reporting which is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Entity {
    Body,
    Local(usize),
    Stmt { block: usize, index: usize },
    Terminator(usize),
}

/// Why a body is malformed; see [`Body::verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Malformed {
    /// A local that the body does not declare.
    UnknownLocal { at: Entity, local: usize },
    /// A jump to a block that the body does not have.
    UnknownBlock { at: Entity, block: usize },
    /// A parameter listed twice, or the result among the parameters.
    ParamReused(usize),
    /// An entity whose origin is not in the table.
    NoOrigin(Entity),
    /// A body with no block to start in.
    Empty,
    /// A jump to the first block, which only a call enters.
    EntryTargeted(usize),
    /// A `yield` resuming where ending the frame goes on.
    YieldEdges(usize),
    /// A frame ending in a body that never suspends.
    Unsuspended(usize),
    /// An assembly block that does not return, followed by more than its block's `Unreachable`.
    PastNever(usize),
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Body => f.write_str("the body"),
            Self::Local(l) => write!(f, "local _{l}"),
            Self::Stmt { block, index } => write!(f, "statement {index} of bb{block}"),
            Self::Terminator(block) => write!(f, "the terminator of bb{block}"),
        }
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownLocal { at, local } => write!(f, "{at} names the undeclared local _{local}"),
            Self::UnknownBlock { at, block } => write!(f, "{at} jumps to the missing block bb{block}"),
            Self::ParamReused(l) => {
                write!(f, "local _{l} is a parameter twice, or both a parameter and the result")
            }
            Self::NoOrigin(at) => write!(f, "{at} has no origin in the table"),
            Self::Empty => f.write_str("the body has no block"),
            Self::EntryTargeted(block) => write!(f, "bb{block} jumps to the first block"),
            Self::YieldEdges(block) => write!(f, "the yield of bb{block} resumes where it is ended"),
            Self::Unsuspended(block) => write!(f, "bb{block} ends a frame the body never suspends"),
            Self::PastNever(block) => {
                write!(f, "bb{block} goes on past an assembly block that does not return")
            }
        }
    }
}

impl std::error::Error for Malformed {}

impl Body {
    /// Checks that everything this body names exists and has an origin in `origins`.
    ///
    /// # Errors
    /// Returns the first violation found.
    pub fn verify(&self, origins: &OriginTable) -> Result<(), Malformed> {
        self.check(Some(origins))
    }

    /// Checks every invariant but that origins are in the table: what each pass leaves, in debug
    /// builds (spec 13, section 4).
    ///
    /// # Errors
    /// Returns the first violation found.
    pub fn verify_structure(&self) -> Result<(), Malformed> {
        self.check(None)
    }

    /// Checks the body after the pass calling this built or rewrote it, in debug builds.
    ///
    /// # Panics
    /// Panics, naming the caller, if the pass left it malformed: a bug of the pass.
    #[track_caller]
    pub(crate) fn verified(&self) {
        if cfg!(debug_assertions)
            && let Err(malformed) = self.verify_structure()
        {
            panic!("a pass left malformed MIR: {malformed}");
        }
    }

    fn check(&self, origins: Option<&OriginTable>) -> Result<(), Malformed> {
        if self.blocks.is_empty() {
            return Err(Malformed::Empty);
        }
        let has = |at: Entity, origin: OriginId| {
            let known = origins.is_none_or(|origins| origins.contains(origin));
            known.then_some(()).ok_or(Malformed::NoOrigin(at))
        };
        let suspends = self.blocks.iter().any(|(_, b)| matches!(b.term.kind, TerminatorKind::Yield { .. }));
        has(Entity::Body, self.origin)?;
        for (local, decl) in self.locals.iter() {
            has(Entity::Local(local.index()), decl.origin)?;
        }
        let mut params = HashSet::new();
        for &local in self.params.iter().chain([&self.ret]) {
            self.local(Entity::Body, local)?;
            if !params.insert(local) {
                return Err(Malformed::ParamReused(local.index()));
            }
        }
        for (block, b) in self.blocks.iter() {
            for (index, stmt) in b.stmts.iter().enumerate() {
                let at = Entity::Stmt { block: block.index(), index };
                has(at, stmt.origin)?;
                self.place(at, &stmt.place)?;
                self.rvalue(at, &stmt.value)?;
            }
            let never = b.stmts.iter().position(|s| matches!(&s.value, Rvalue::Asm(asm) if !asm.returns));
            if never.is_some_and(|i| i + 1 != b.stmts.len() || b.term.kind != TerminatorKind::Unreachable) {
                return Err(Malformed::PastNever(block.index()));
            }
            let at = Entity::Terminator(block.index());
            has(at, b.term.origin)?;
            if b.term.kind.successors().contains(&BlockId::new(0)) {
                return Err(Malformed::EntryTargeted(block.index()));
            }
            match &b.term.kind {
                TerminatorKind::Ended if !suspends => return Err(Malformed::Unsuspended(block.index())),
                TerminatorKind::Yield { resume, cancel, .. } if resume == cancel => {
                    return Err(Malformed::YieldEdges(block.index()));
                }
                TerminatorKind::Return | TerminatorKind::Ended | TerminatorKind::Unreachable => {}
                TerminatorKind::Goto(to) => self.block(at, *to)?,
                TerminatorKind::Yield { value, resume, cancel } => {
                    self.operand(at, value)?;
                    self.block(at, *resume)?;
                    self.block(at, *cancel)?;
                }
                TerminatorKind::Branch { cond, then, otherwise } => {
                    self.operand(at, cond)?;
                    self.block(at, *then)?;
                    self.block(at, *otherwise)?;
                }
                TerminatorKind::Defer { consumes, run, skip } => {
                    for &local in consumes {
                        self.local(at, local)?;
                    }
                    self.block(at, *run)?;
                    self.block(at, *skip)?;
                }
            }
        }
        Ok(())
    }

    fn local(&self, at: Entity, local: Local) -> Result<(), Malformed> {
        self.locals.get(local).map(drop).ok_or(Malformed::UnknownLocal { at, local: local.index() })
    }

    fn block(&self, at: Entity, block: BlockId) -> Result<(), Malformed> {
        self.blocks.get(block).map(drop).ok_or(Malformed::UnknownBlock { at, block: block.index() })
    }

    fn operand(&self, at: Entity, operand: &Operand) -> Result<(), Malformed> {
        match operand {
            Operand::Place(place) => self.place(at, place),
            Operand::Const(_) => Ok(()),
        }
    }

    /// A place's local, and the locals its indices read.
    fn place(&self, at: Entity, place: &Place) -> Result<(), Malformed> {
        self.local(at, place.local)?;
        place.projection.iter().try_for_each(|p| match p {
            Projection::Index(local) => self.local(at, *local),
            Projection::Deref
            | Projection::Field(_)
            | Projection::Position(_)
            | Projection::Payload { .. } => Ok(()),
        })
    }

    /// The places and operands an rvalue reads, and the places it writes besides the statement's.
    fn rvalue(&self, at: Entity, rvalue: &Rvalue) -> Result<(), Malformed> {
        if let Rvalue::Ref { place, .. }
        | Rvalue::IsVariant { place, .. }
        | Rvalue::Resume { frame: place, .. }
        | Rvalue::End { frame: place } = rvalue
        {
            self.place(at, place)?;
        }
        rvalue.operands().into_iter().try_for_each(|o| self.operand(at, o))?;
        rvalue.outputs().into_iter().try_for_each(|(p, _)| self.place(at, p))
    }
}
