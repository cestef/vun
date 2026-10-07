//! Guaranteed tail calls (spec 09, section 3). Cranelift jumps only between functions of its
//! `tail` calling convention, which nothing else can call, so each function that tail calls or is
//! tail called is compiled as that body and an entry of the target's convention calling it
//! (`func::stub`), which everything else reaches it through. A group of functions that tail call
//! one another is compiled this way whole or not at all: where another backend compiles one, its
//! jump to the entry of a neighbor would grow the stack, so the call is refused and the assisting
//! backend compiles the function.

use vun_index::{Idx, IndexVec};
use vun_lir::analysis::Reached;
use vun_lir::{CallKind, Callee, Conv, FuncId, Function, InstKind, Unit};

/// The functions compiled as a body in the tail convention and an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tails(IndexVec<FuncId, bool>);

/// What `unit` has of the functions a tail call joins, as groups.
struct Groups {
    parent: Vec<usize>,
    /// Whether anything keeps the group of a function out of the tail convention.
    held_out: Vec<bool>,
}

impl Groups {
    fn root(&mut self, mut at: usize) -> usize {
        while self.parent[at] != at {
            self.parent[at] = self.parent[self.parent[at]];
            at = self.parent[at];
        }
        at
    }

    fn join(&mut self, a: FuncId, b: FuncId) {
        let (a, b) = (self.root(a.index()), self.root(b.index()));
        self.parent[a] = b;
        self.held_out[b] |= self.held_out[a];
    }

    fn refuse(&mut self, f: FuncId) {
        let root = self.root(f.index());
        self.held_out[root] = true;
    }

    fn refused(&mut self, f: FuncId) -> bool {
        let root = self.root(f.index());
        self.held_out[root]
    }
}

impl Tails {
    pub(crate) fn contains(&self, f: FuncId) -> bool {
        self.0.get(f).copied().unwrap_or(false)
    }

    /// The functions of `unit` reached that tail call or are tail called, but those whose group
    /// holds a function another backend compiles (`lacking`).
    pub(crate) fn of(unit: &Unit, reached: &Reached, lacking: &[FuncId]) -> Self {
        let count = unit.functions.len();
        let mut groups = Groups { parent: (0..count).collect(), held_out: vec![false; count] };
        let mut joined = vec![false; count];
        let reachable: Vec<_> = unit.functions.iter().filter(|&(id, _)| reached.contains(id)).collect();
        for &(id, f) in &reachable {
            if !Self::fits(f) {
                vun_trace::tracing::trace!(
                    id = id.index(),
                    conv = ?f.signature.conv,
                    "held out of the tail convention"
                );
                groups.refuse(id);
            }
        }
        for &(id, f) in &reachable {
            for kind in f.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &f.insts[i].kind) {
                match kind {
                    InstKind::Call { callee: Callee::Func(g), kind: CallKind::Tail, .. } => {
                        groups.join(id, *g);
                        joined[id.index()] = true;
                        joined[g.index()] = true;
                    }
                    InstKind::Call { kind: CallKind::Tail, .. } => groups.refuse(id),
                    _ => {}
                }
            }
        }
        for &f in lacking {
            vun_trace::tracing::debug!(
                id = f.index(),
                "compiled elsewhere: its group is held out of the tail convention"
            );
            groups.refuse(f);
        }
        Self(unit.functions.iter().map(|(id, _)| joined[id.index()] && !groups.refused(id)).collect())
    }

    /// Whether the body of `f` can be passed as the tail convention does: its values are placed
    /// as Cranelift places them.
    fn fits(f: &Function) -> bool {
        f.signature.conv == Conv::Vun && f.signature.convention.is_none() && f.signature.variadic.is_none()
    }
}
