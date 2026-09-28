//! Simplifying a unit after lowering, one function at a time, callees first: slots of
//! aggregates cut into scalars, locals promoted to values, operations on the constants that
//! makes folded, constants hoisted out of loops, computations repeated reused, jumps threaded
//! through the decisions that folds, truncations narrowed, and what nothing reads swept, round
//! after round while a round changes something; then the small functions it calls inlined, simplified already, and the
//! rounds run again. A function is done before any caller decides whether it is small.

#[cfg(test)]
mod tests;

use crate::{Fold, Hoist, Inline, Narrow, Promote, Reuse, Scalarize, Sweep, Thread};
use vun_index::Idx;
use vun_par::Parallel;
use vun_trace::{Activity, tracing};

use vun_lir::func::Function;
use vun_lir::unit::Unit;

const SIMPLIFY: Activity = Activity::stage("simplify");

/// The most rounds of the passes over one function: each round mostly finishes what the one
/// before exposed, and few are left after three.
const ROUNDS: usize = 4;

pub trait Simplify {
    /// Simplifies every function (see the module).
    fn simplify(&mut self);

    /// Callees first, each function inlines its small callees, simplified already, and is
    /// tidied again when it did: what simplifying does once every function is tidied.
    fn inline_small(&mut self);
}

impl Simplify for Unit {
    /// Every function tidied at once over threads, since tidying reads no other function; then,
    /// callees first, each inlines its small callees, simplified already, and is tidied again.
    fn simplify(&mut self) {
        let _timer = SIMPLIFY.start();
        let bits = self.pointer_bits;
        let functions: Vec<Function> = std::mem::take(&mut self.functions).into_vec();
        let tidied = functions.par_map(|f| {
            let mut f = f.clone();
            let started = std::time::Instant::now();
            f.tidy(bits);
            tracing::debug!(origin = ?f.origin, insts = f.insts.len(), micros = started.elapsed().as_micros(), "tidied");
            f
        });
        self.functions = tidied.into_iter().collect();
        self.inline_small();
    }

    fn inline_small(&mut self) {
        let bits = self.pointer_bits;
        for f in self.callees_first() {
            if timed(INLINE, || self.inline_into(f)) > 0 {
                let started = std::time::Instant::now();
                self.functions[f].tidy(bits);
                let (insts, micros) = (self.functions[f].insts.len(), started.elapsed().as_micros());
                tracing::debug!(f = f.index(), insts, micros, "tidied after inlining");
            }
        }
        timed(VERIFY, || {
            self.verified();
            0
        });
    }
}

pub trait Tidy {
    /// Runs `Function::scalarize`, `promote`, `fold`, `hoist`, `reuse`, `thread`, `narrow` and
    /// `sweep` in rounds until one changes nothing.
    fn tidy(&mut self, pointer_bits: u32);
}

impl Tidy for Function {
    fn tidy(&mut self, pointer_bits: u32) {
        for _ in 0..ROUNDS {
            let changed = timed(SCALARIZE, || self.scalarize(pointer_bits))
                + timed(PROMOTE, || self.promote(pointer_bits))
                + timed(FOLD, || self.fold())
                + timed(HOIST, || self.hoist())
                + timed(REUSE, || self.reuse())
                + timed(THREAD, || self.thread(pointer_bits))
                + timed(NARROW, || self.narrow(pointer_bits))
                + timed(SWEEP, || self.sweep());
            if changed == 0 {
                return;
            }
        }
    }
}

const SCALARIZE: Activity = Activity::stage("scalarize");
const PROMOTE: Activity = Activity::stage("promote");
const FOLD: Activity = Activity::stage("fold");
const HOIST: Activity = Activity::stage("hoist");
const REUSE: Activity = Activity::stage("reuse");
const THREAD: Activity = Activity::stage("thread");
const NARROW: Activity = Activity::stage("narrow");
const SWEEP: Activity = Activity::stage("sweep");
const INLINE: Activity = Activity::stage("inline");
const VERIFY: Activity = Activity::stage("verify");

/// Runs the pass `run`, timed as `pass`: what it changed.
fn timed(pass: Activity, run: impl FnOnce() -> usize) -> usize {
    let _timer = pass.start();
    let started = std::time::Instant::now();
    let changed = run();
    tracing::trace!(pass = pass.name, changed, micros = started.elapsed().as_micros(), "pass");
    changed
}
