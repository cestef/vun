//! The function being made: its locals, which of them are visible, and what it has cost.

use vun_index::IndexVec;

use crate::ir::{Label, Local, LocalId};
use crate::ty::Ty;

/// A loop forever being made, which a `break` with a value leaves.
pub(super) struct Exit {
    pub(super) label: Label,
    /// What the loop's value is wanted to be.
    pub(super) want: Ty,
    /// The types of the values broken out with.
    pub(super) found: Vec<Ty>,
}

/// A function under construction.
pub(super) struct Scope {
    pub(super) locals: IndexVec<LocalId, Local>,
    /// The locals a form may read, in the order they were bound.
    pub(super) visible: Vec<LocalId>,
    /// The steps the function takes, as far as made.
    pub(super) steps: u64,
    /// How many times the statements being made run for each run of the function.
    pub(super) runs: u64,
    /// How many counted loops enclose them.
    pub(super) loops: usize,
    /// The locals an enclosing loop steps through, which nothing in it may change.
    pub(super) frozen: Vec<LocalId>,
    /// Whether they are the body of a closure.
    pub(super) inside_closure: bool,
    /// How many blocks enclose them.
    pub(super) nesting: usize,
    /// The named loops around them that a `break` or `continue` may name.
    pub(super) labels: Vec<Label>,
    /// The loops forever around them, the innermost last.
    pub(super) exits: Vec<Exit>,
    /// How many loops the function has named.
    pub(super) named: usize,
    /// What the iterator function being made yields, where its statements may yield.
    pub(super) yields: Option<Ty>,
}

impl Default for Scope {
    fn default() -> Self {
        Self {
            locals: IndexVec::new(),
            visible: Vec::new(),
            steps: 0,
            runs: 1,
            loops: 0,
            frozen: Vec::new(),
            inside_closure: false,
            nesting: 0,
            labels: Vec::new(),
            exits: Vec::new(),
            named: 0,
            yields: None,
        }
    }
}

impl Scope {
    /// Binds a new local, visible from here to the end of the block.
    pub(super) fn bind(&mut self, ty: Ty, mutable: bool) -> LocalId {
        let id = self.locals.push(Local { ty, mutable });
        self.visible.push(id);
        id
    }

    /// The visible locals whose type satisfies `keep`.
    pub(super) fn find(&self, keep: impl Fn(&Local) -> bool) -> Vec<LocalId> {
        self.visible.iter().copied().filter(|&id| keep(&self.locals[id])).collect()
    }

    /// A name for a loop that no other has.
    pub(super) fn name_loop(&mut self) -> Label {
        self.named += 1;
        Label(self.named - 1)
    }

    /// Takes one more step, as often as the statements being made run.
    pub(super) fn step(&mut self) {
        self.steps = self.steps.saturating_add(self.runs);
    }
}
