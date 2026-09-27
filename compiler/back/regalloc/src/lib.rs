//! The register allocator the native backends share (spec 13, section 8.6): linear scan on SSA
//! after Wimmer and Franz (2010) over a LIR function, from what its architecture describes as
//! data (`Machine`, `Describe`), giving a plan (`Allocation`) that the single-pass framework
//! follows and that `Allocation::check` verifies by simulation.

mod build;
mod check;
mod interval;
mod live;
mod machine;
mod needs;
mod plan;
mod points;
mod resolve;
#[cfg(any(test, feature = "sample"))]
pub mod sample;
mod scan;
mod split;

#[cfg(test)]
mod tests;

pub use check::{Why, Wrong};
pub use machine::{Bank, Machine, PReg, RegSet};
pub use needs::{Describe, Need, Needs, Out, Read};
pub use plan::{Allocation, Loc, Move, Piece, Source};
pub use points::{Point, Points, Pos};

use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Loops, Order};
use vun_lir::{Function, Value};

use build::Built;
use interval::IntervalId;
use scan::Scan;
use split::Splits;

impl Allocation {
    /// Plans where every value of `f` lives.
    #[must_use]
    pub fn of(f: &Function, d: &impl Describe) -> Self {
        let cfg = Cfg::of(f);
        let loops = Loops::of(f, &cfg);
        let order = Order::of(f, &loops);
        let points = Points::of(f, &order);
        let built = Built::of(f, d, &cfg, &points);
        let splits = Splits::of(&points, &loops);
        let hints = Self::hints(f, &points, &built);
        let intervals = Scan::new(d.machine(), &splits, &built)
            .run(|scan, id| hints.get(id).into_iter().flatten().find_map(|&(v, pos)| scan.reg_of(v, pos)));
        Self::resolve(f, d, points, built, &intervals)
    }

    /// For each interval, where the values whose register it would rather have are: the operand
    /// a result overwrites, and the arguments jumps pass to a block parameter.
    fn hints(f: &Function, points: &Points, built: &Built) -> IndexVec<IntervalId, Vec<(Value, Pos)>> {
        let mut hints = IndexVec::filled(Vec::new(), built.intervals.len());
        for &(id, operand) in &built.follows {
            hints[id].push((operand, built.intervals[id].start().before()));
        }
        for &block in &points.order {
            let last = points.blocks[block].expect("an emitted block").1;
            for target in f.blocks[block].term.targets() {
                for (&param, &arg) in f.blocks[target.block].params.iter().zip(&target.args) {
                    if let Some(id) = built.of_value[param] {
                        hints[id].push((arg, Pos::read(last)));
                    }
                }
            }
        }
        hints
    }
}
