//! Worst-case stack use (spec 07, section 4): a function's frame and the most any call it makes
//! takes, over the unit's call graph, from the frame size of each function and the bound each
//! import's declaration states.

use vun_hash::Map;

use vun_codec::{Decode, Encode};
use vun_diag::Msg;
use vun_index::IndexVec;
use vun_origin::OriginId;

use crate::{CallKind, Callee, FuncId, ImportId, InstKind, Unit};

/// Why a function's stack use has no bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unbounded {
    /// It reaches a call of this function made while a call of it runs.
    Recursive(FuncId),
    /// It calls this import, whose frames are not the unit's, and whose declaration states no
    /// bound.
    Import(ImportId),
    /// It calls a function through an address.
    Indirect(FuncId),
    /// No frame size was reported for this function.
    Unreported(FuncId),
}

impl Unbounded {
    /// Where the unbounded use arises in `unit`: the recursive function, the import, the function
    /// calling through an address, or the one without a frame size.
    #[must_use]
    pub fn at(self, unit: &Unit) -> OriginId {
        match self {
            Self::Recursive(f) | Self::Indirect(f) | Self::Unreported(f) => unit.functions[f].origin,
            Self::Import(i) => unit.imports[i].origin,
        }
    }

    /// What leaves the use unbounded, said of the function reaching it.
    #[must_use]
    pub fn msg(self) -> Msg {
        match self {
            Self::Recursive(_) => Msg::new().text("a function it reaches is called again while it runs"),
            Self::Import(_) => Msg::new()
                .text("it reaches a function of another program whose declaration states no ")
                .code("stack.max"),
            Self::Indirect(_) => Msg::new().text("a function it reaches calls through an address"),
            Self::Unreported(_) => Msg::new().text("no frame size was reported for a function it reaches"),
        }
    }
}

/// The worst-case stack use of each function of a unit, in bytes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StackUse(IndexVec<FuncId, Result<u64, Unbounded>>);

/// How far the walk of one function has got.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode)]
enum Walk {
    Unseen,
    Open,
    Done(Result<u64, Unbounded>),
}

impl StackUse {
    /// The stack use of every function of `unit`, whose frame sizes `frames` gives.
    #[must_use]
    pub fn of(unit: &Unit, frames: &[(FuncId, u64)]) -> Self {
        let frames: Map<FuncId, u64> = frames.iter().copied().collect();
        let count = unit.functions.len();
        let mut walk = Walker { unit, frames, state: IndexVec::filled(Walk::Unseen, count) };
        let uses: Vec<_> = unit.functions.iter().map(|(f, _)| walk.visit(f)).collect();
        Self(uses.into())
    }

    /// The most bytes of stack a call of `f` takes, with every call it makes.
    ///
    /// # Errors
    /// Returns why it has no bound.
    pub fn of_function(&self, f: FuncId) -> Result<u64, Unbounded> {
        self.0[f]
    }
}

struct Walker<'u> {
    unit: &'u Unit,
    frames: Map<FuncId, u64>,
    state: IndexVec<FuncId, Walk>,
}

impl Walker<'_> {
    fn visit(&mut self, f: FuncId) -> Result<u64, Unbounded> {
        match self.state[f] {
            Walk::Done(result) => return result,
            Walk::Open => return Err(Unbounded::Recursive(f)),
            Walk::Unseen => {}
        }
        self.state[f] = Walk::Open;
        let result = self.deepest(f);
        self.state[f] = Walk::Done(result);
        result
    }

    /// The most bytes of stack a call of `f` takes: the frame of each function it reaches by tail
    /// calls alone, which replace one another, and the deepest plain call each makes.
    fn deepest(&mut self, f: FuncId) -> Result<u64, Unbounded> {
        let unit = self.unit;
        let mut reached = vec![f];
        let mut worst = 0;
        let mut next = 0;
        while let Some(&h) = reached.get(next) {
            next += 1;
            let frame = self.frames.get(&h).copied().ok_or(Unbounded::Unreported(h))?;
            let mut deepest = 0;
            for (_, inst) in unit.functions[h].insts.iter() {
                let InstKind::Call { callee, kind, .. } = &inst.kind else { continue };
                let used = match callee {
                    Callee::Func(g) if *kind == CallKind::Tail => {
                        if !reached.contains(g) {
                            reached.push(*g);
                        }
                        continue;
                    }
                    Callee::Func(g) => self.visit(*g)?,
                    Callee::Import(i) => unit.imports[*i].stack_max.ok_or(Unbounded::Import(*i))?,
                    Callee::Indirect(..) => return Err(Unbounded::Indirect(h)),
                };
                if *kind == CallKind::Tail {
                    worst = worst.max(used);
                } else {
                    deepest = deepest.max(used);
                }
            }
            worst = worst.max(frame + deepest);
        }
        Ok(worst)
    }
}
