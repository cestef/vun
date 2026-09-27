//! The checker: simulates the plan over the control-flow graph, what each register and home
//! holds through moves, definitions, clobbers and scratch registers, meeting at joins by what
//! every predecessor agrees on, and requires each use to read the place holding its value.

use std::collections::HashMap;
use std::fmt;

use vun_index::IndexVec;
use vun_lir::{Block, Function, Value};

use crate::needs::{Describe, Need, Needs, Out, Read, Remade};
use crate::plan::{Allocation, Loc, Move, Source};
use crate::points::Pos;

/// What the checker finds wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The plan has no place for a value where it is read.
    Placeless,
    /// The place the plan gives does not hold the value.
    Elsewhere { at: Loc, found: Option<Value> },
    /// A value read or written where a register is needed is not in one.
    NotInReg(Loc),
    /// An operand held in a register the point takes for itself or for scratch.
    Reserved(Loc),
    /// An operand read after the result is written, held where the result is.
    Overwritten(Loc),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wrong {
    pub point: u32,
    pub value: Value,
    pub why: Why,
}

impl fmt::Display for Wrong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "point {}, {}: {:?}", self.point, self.value, self.why)
    }
}

impl std::error::Error for Wrong {}

/// What each place holds, where known.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State(HashMap<Loc, Value>);

impl State {
    /// Requires `at` to hold `v` where `point` reads it; the home of a value made again holds it
    /// by making it.
    fn expect(&self, d: &impl Describe, at: Loc, v: Value, point: u32) -> Result<(), Wrong> {
        let found = self.0.get(&at).copied();
        if found == Some(v) || (at == Loc::Home(v) && d.remade(v) == Some(Remade::Held)) {
            Ok(())
        } else {
            Err(Wrong { point, value: v, why: Why::Elsewhere { at, found } })
        }
    }

    fn apply(&mut self, moves: &[Move]) {
        for m in moves {
            let value = match m.from {
                Source::At(at) => self.0.get(&at).copied(),
                Source::Remade(v) => Some(v),
            };
            match value {
                Some(v) => self.0.insert(m.to, v),
                None => self.0.remove(&m.to),
            };
        }
    }

    /// What `self` and `other` agree on.
    fn meet(&self, other: &Self) -> Self {
        Self(self.0.iter().filter(|&(l, v)| other.0.get(l) == Some(v)).map(|(&l, &v)| (l, v)).collect())
    }
}

impl Allocation {
    /// Checks the plan against what `f` reads and writes.
    ///
    /// # Errors
    /// Returns the first use that would read the wrong value.
    ///
    /// # Panics
    /// Never: every block a jump reaches is numbered.
    pub fn check(&self, f: &Function, d: &impl Describe) -> Result<(), Wrong> {
        let (points, live) = (&self.points, &self.live);
        let mut entries: IndexVec<Block, Option<State>> = IndexVec::filled(None, f.blocks.len());
        let Some(entry) = f.entry() else { return Ok(()) };
        entries[entry] = Some(State::default());
        let mut work = vec![entry];
        let mut needs = Needs::default();
        while let Some(block) = work.pop() {
            let mut state = entries[block].clone().expect("queued with a state");
            let (first, last) = points.blocks[block].expect("an emitted block");
            for point in first..=last {
                points.needs(f, d, point, &mut needs);
                self.point(d, point, &needs, &mut state)?;
            }
            for target in f.blocks[block].term.targets() {
                let to = target.block;
                let mut there = state.clone();
                there.apply(self.edge(block, to));
                let entered = points.entered(to);
                for (&param, &arg) in f.blocks[to].params.iter().zip(&target.args) {
                    let Some(at) = self.at(param, entered) else { continue };
                    there.expect(d, at, arg, last)?;
                    there.0.insert(at, param);
                }
                for v in live.into[to].iter() {
                    let at =
                        self.at(v, entered).ok_or(Wrong { point: last, value: v, why: Why::Placeless })?;
                    there.expect(d, at, v, last)?;
                }
                let merged = match &entries[to] {
                    None => there,
                    Some(known) => known.meet(&there),
                };
                if entries[to].as_ref() != Some(&merged) {
                    entries[to] = Some(merged);
                    work.push(to);
                }
            }
        }
        Ok(())
    }

    fn point(&self, d: &impl Describe, point: u32, needs: &Needs, state: &mut State) -> Result<(), Wrong> {
        let (read, write) = (Pos::read(point), Pos::write(point));
        state.apply(self.before(point));
        // Registers the point writes while it may still read: its own and its scratch ones.
        let taken = self.temps(point).iter().fold(needs.reserved, |set, &r| set.with(r));
        let reused =
            needs.defs.iter().find_map(|&(_, out)| if let Out::Reuse(i) = out { Some(i) } else { None });
        let results: Vec<Loc> = needs.defs.iter().filter_map(|&(v, _)| self.at(v, write)).collect();
        for (i, &Read { value: v, need, late }) in needs.uses.iter().enumerate() {
            if d.recomputed(v) {
                continue;
            }
            let at = match need {
                Need::Fixed(r) => Loc::Reg(r),
                Need::Reg | Need::Any => {
                    self.at(v, read).ok_or(Wrong { point, value: v, why: Why::Placeless })?
                }
            };
            let late = late || reused.is_some_and(|r| r != i);
            let why = if need == Need::Reg && !matches!(at, Loc::Reg(_)) {
                Some(Why::NotInReg(at))
            } else if matches!(at, Loc::Reg(r) if taken.contains(r)) {
                Some(Why::Reserved(at))
            } else if late && results.contains(&at) {
                Some(Why::Overwritten(at))
            } else {
                None
            };
            if let Some(why) = why {
                return Err(Wrong { point, value: v, why });
            }
            state.expect(d, at, v, point)?;
        }
        let written = needs.clobbers.union(needs.reserved).iter().chain(self.temps(point).iter().copied());
        for r in written {
            state.0.remove(&Loc::Reg(r));
        }
        state.0.remove(&Loc::Temp);
        for &(v, out) in &needs.defs {
            let at = match out {
                Out::Fixed(r) => Loc::Reg(r),
                Out::Home => Loc::Home(v),
                Out::Reg | Out::Reuse(_) | Out::Any => {
                    self.at(v, write).ok_or(Wrong { point, value: v, why: Why::Placeless })?
                }
            };
            if matches!(out, Out::Reg | Out::Reuse(_)) && !matches!(at, Loc::Reg(_)) {
                return Err(Wrong { point, value: v, why: Why::NotInReg(at) });
            }
            state.0.insert(at, v);
        }
        state.apply(self.after(point));
        Ok(())
    }
}
