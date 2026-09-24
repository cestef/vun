//! The verifier of MIR: every local, place and jump target a body names exists, parameters are
//! distinct locals apart from the result, and every entity has an origin in the table (spec 13,
//! section 4).

use std::collections::HashSet;
use std::fmt;

use overt_index::Idx;
use overt_origin::{OriginId, OriginTable};

use crate::body::{BlockId, Body, Local, Operand, Rvalue, TerminatorKind};
use crate::place::{Place, Projection};

/// The entities of a body, for reporting which is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entity {
    Body,
    Local(usize),
    Stmt { block: usize, index: usize },
    Terminator(usize),
}

/// Why a body is malformed; see [`Body::verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        if self.blocks.is_empty() {
            return Err(Malformed::Empty);
        }
        let has = |at: Entity, origin: OriginId| {
            origins.contains(origin).then_some(()).ok_or(Malformed::NoOrigin(at))
        };
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
            let at = Entity::Terminator(block.index());
            has(at, b.term.origin)?;
            match &b.term.kind {
                TerminatorKind::Return | TerminatorKind::Unreachable => {}
                TerminatorKind::Goto(to) => self.block(at, *to)?,
                TerminatorKind::Branch { cond, then, otherwise } => {
                    self.operand(at, cond)?;
                    self.block(at, *then)?;
                    self.block(at, *otherwise)?;
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
            Projection::Deref | Projection::Field(_) | Projection::Position(_) | Projection::Payload(_) => {
                Ok(())
            }
        })
    }

    fn rvalue(&self, at: Entity, rvalue: &Rvalue) -> Result<(), Malformed> {
        let operands: Vec<&Operand> = match rvalue {
            Rvalue::Use(o)
            | Rvalue::Unary { operand: o, .. }
            | Rvalue::Narrow { operand: o, .. }
            | Rvalue::Repeat { operand: o, .. } => vec![o],
            Rvalue::Ref { place, .. } | Rvalue::IsVariant { place, .. } => return self.place(at, place),
            Rvalue::Aggregate { parts, .. } => parts.iter().collect(),
            Rvalue::Binary { left, right, .. } => vec![left, right],
            Rvalue::Range { lo, hi, .. } => [lo].into_iter().chain(hi).collect(),
            Rvalue::Call { callee, args } => [callee].into_iter().chain(args).collect(),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } => Vec::new(),
        };
        operands.into_iter().try_for_each(|o| self.operand(at, o))
    }
}
