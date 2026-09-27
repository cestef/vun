//! The linear order of a function: each block's entry, where its parameters are defined, its
//! instructions and its terminator, the blocks in `Order`. Each point has two positions: where
//! it reads its operands, then where it writes its results.

use vun_index::IndexVec;
use vun_lir::analysis::Order;
use vun_lir::{Block, Function, Inst};

use crate::needs::{Describe, Need, Needs, Out};

/// A place in the linear order where values are read or written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    Entry(Block),
    Inst(Inst),
    Term(Block),
}

/// A position: the read (even) or the write (odd) of a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pos(pub u32);

impl Pos {
    pub const MAX: Self = Self(u32::MAX);

    #[must_use]
    pub const fn read(point: u32) -> Self {
        Self(2 * point)
    }

    #[must_use]
    pub const fn write(point: u32) -> Self {
        Self(2 * point + 1)
    }

    /// The point this is a position of.
    #[must_use]
    pub const fn point(self) -> u32 {
        self.0 / 2
    }

    /// The position after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The read of this position's point: where moves before it run.
    #[must_use]
    pub const fn before(self) -> Self {
        Self(self.0 & !1)
    }
}

/// The points of a function and where each block's are.
pub struct Points {
    pub points: Vec<Point>,
    /// The first point (its entry) and the last (its terminator) of each block emitted.
    pub blocks: IndexVec<Block, Option<(u32, u32)>>,
    pub insts: IndexVec<Inst, Option<u32>>,
    /// The blocks in the order they are numbered.
    pub order: Vec<Block>,
}

impl Points {
    /// # Panics
    /// Panics past `u32::MAX / 2` points.
    #[must_use]
    pub fn of(f: &Function, order: &Order) -> Self {
        let mut points = Vec::new();
        let mut blocks = IndexVec::filled(None, f.blocks.len());
        let mut insts = IndexVec::filled(None, f.insts.len());
        let index = |points: &Vec<Point>| u32::try_from(points.len()).expect("points fit u32");
        for &block in order.blocks() {
            let first = index(&points);
            points.push(Point::Entry(block));
            for &inst in &f.blocks[block].insts {
                insts[inst] = Some(index(&points));
                points.push(Point::Inst(inst));
            }
            blocks[block] = Some((first, index(&points)));
            points.push(Point::Term(block));
        }
        Self { points, blocks, insts, order: order.blocks().to_vec() }
    }

    /// The first position of `block`.
    ///
    /// # Panics
    /// Panics for a block not emitted.
    #[must_use]
    pub fn start(&self, block: Block) -> Pos {
        Pos::read(self.blocks[block].expect("an emitted block").0)
    }

    /// Where `block` is entered: once its parameters are defined, which the jumps to it do, so
    /// the values live into it are where the jumps leave them.
    ///
    /// # Panics
    /// Panics for a block not emitted.
    #[must_use]
    pub fn entered(&self, block: Block) -> Pos {
        Pos::write(self.blocks[block].expect("an emitted block").0)
    }

    /// The position past the last of `block`.
    ///
    /// # Panics
    /// Panics for a block not emitted.
    #[must_use]
    pub fn end(&self, block: Block) -> Pos {
        Pos::write(self.blocks[block].expect("an emitted block").1).next()
    }

    /// What the point at `point` needs: an instruction's operands and result, a terminator's
    /// own operands and the arguments of its jumps, which may be anywhere, or the parameters
    /// a block's entry defines, where the function receives them for the first block.
    pub fn needs(&self, f: &Function, d: &impl Describe, point: u32, needs: &mut Needs) {
        needs.clear();
        match self.points[point as usize] {
            Point::Inst(inst) => {
                d.inst(f, inst, needs);
                needs.homeless(d);
            }
            Point::Term(block) => {
                d.term(f, block, needs);
                needs.homeless(d);
                let args = f.blocks[block].term.targets().flat_map(|t| t.args.iter().copied());
                for v in args.filter(|&v| !d.recomputed(v)) {
                    needs.read(v, Need::Any);
                }
            }
            Point::Entry(block) if Some(block) == f.entry() => d.entry(f, needs),
            Point::Entry(block) => needs.defs.extend(f.blocks[block].params.iter().map(|&p| (p, Out::Any))),
        }
    }

    /// Whether `pos` is a block's entry, where the jumps to it place values.
    #[must_use]
    pub fn at_entry(&self, pos: Pos) -> bool {
        matches!(self.points.get(pos.point() as usize), Some(Point::Entry(_)))
    }
}
