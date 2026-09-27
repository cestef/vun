//! The plan the framework follows: where each value is at every position, the moves before and
//! after each point and on each edge, the scratch registers of each point, and the registers
//! used at all.

use std::collections::HashMap;
use std::fmt;

use vun_index::IndexVec;
use vun_lir::{Block, Value};

use crate::live::Live;
use crate::machine::{PReg, RegSet};
use crate::points::{Point, Points, Pos};

/// Where a value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Loc {
    Reg(PReg),
    /// The value's home in the frame; for a value made again (`Remade::Held`), nowhere: moves
    /// out of it make the value, and none go into it.
    Home(Value),
    /// The framework's own register for breaking a cycle of moves.
    Temp,
}

/// Where a move reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    At(Loc),
    /// A value recomputed where it is used: a constant or a known address.
    Remade(Value),
}

/// A copy of `value`, which `from` holds or recomputes, into `to`. On an edge, a move into a
/// block parameter's place copies its argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Source,
    pub to: Loc,
    pub value: Value,
}

/// Where one value is from a position until another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub from: Pos,
    pub to: Pos,
    pub at: Loc,
}

pub struct Allocation {
    pub(crate) pieces: IndexVec<Value, Vec<Piece>>,
    pub(crate) before: HashMap<u32, Vec<Move>>,
    pub(crate) after: HashMap<u32, Vec<Move>>,
    pub(crate) edges: HashMap<(Block, Block), Vec<Move>>,
    pub(crate) temps: HashMap<u32, Vec<PReg>>,
    /// Every register a value or scratch register is given.
    pub used: RegSet,
    /// The points of the function, which the moves and scratch registers are by.
    pub points: Points,
    pub(crate) live: Live,
}

impl Allocation {
    /// Where `v` is at `pos`; `None` for a value recomputed where used, or one not live there.
    #[must_use]
    pub fn at(&self, v: Value, pos: Pos) -> Option<Loc> {
        let pieces = self.pieces.get(v)?;
        pieces.iter().find(|p| p.from <= pos && pos < p.to).map(|p| p.at)
    }

    /// The moves run before the point `point` reads its operands, in order.
    #[must_use]
    pub fn before(&self, point: u32) -> &[Move] {
        self.before.get(&point).map_or(&[], Vec::as_slice)
    }

    /// The moves run after the point `point` writes its results, in order.
    #[must_use]
    pub fn after(&self, point: u32) -> &[Move] {
        self.after.get(&point).map_or(&[], Vec::as_slice)
    }

    /// The moves run on the edge from `from` to `to`, in order.
    #[must_use]
    pub fn edge(&self, from: Block, to: Block) -> &[Move] {
        self.edges.get(&(from, to)).map_or(&[], Vec::as_slice)
    }

    /// The scratch registers the point `point` takes, in the order it asked for them.
    #[must_use]
    pub fn temps(&self, point: u32) -> &[PReg] {
        self.temps.get(&point).map_or(&[], Vec::as_slice)
    }
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reg(r) => r.fmt(f),
            Self::Home(v) => write!(f, "[{v}]"),
            Self::Temp => f.write_str("temp"),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::At(at) => at.fmt(f),
            Self::Remade(v) => write!(f, "={v}"),
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} <- {} ({})", self.to, self.from, self.value)
    }
}

/// The plan as `vun debug` prints it: each value's places, then each point with its moves and
/// scratch registers.
impl fmt::Display for Allocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (v, pieces) in self.pieces.iter().filter(|(_, p)| !p.is_empty()) {
            write!(f, "{v}:")?;
            for p in pieces {
                write!(f, " {}..{} {}", p.from.0, p.to.0, p.at)?;
            }
            writeln!(f)?;
        }
        let list = |f: &mut fmt::Formatter<'_>, what: &str, moves: &[Move]| -> fmt::Result {
            moves.iter().try_for_each(|m| writeln!(f, "    {what} {m}"))
        };
        for (point, at) in self.points.points.iter().enumerate() {
            let point = u32::try_from(point).expect("points fit u32");
            list(f, "before", self.before(point))?;
            writeln!(
                f,
                "{} {at:?}{}",
                Pos::read(point).0,
                if self.temps(point).is_empty() {
                    String::new()
                } else {
                    format!(" temps {:?}", self.temps(point))
                }
            )?;
            list(f, "after", self.after(point))?;
            if let Point::Term(block) = at {
                for ((from, to), moves) in self.edges.iter().filter(|((from, _), _)| from == block) {
                    list(f, &format!("{from}->{to}"), moves)?;
                }
            }
        }
        Ok(())
    }
}
