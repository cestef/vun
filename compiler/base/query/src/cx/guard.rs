//! What a computation and a verification hold on the thread's stack, released when they end and
//! when a panic unwinds past them: a context going on after a panic it caught sees no frame of a
//! computation that never finished, which it would take for a cycle.

use vun_trace::Timer;

use super::Cx;
use super::frame::Standing;
use crate::memo::Dep;
use crate::table::{Slot, TableId};

/// A computation on the stack.
pub(super) struct Entered<'c, 'db> {
    cx: &'c Cx<'db>,
    timer: Option<Timer>,
}

impl<'c, 'db> Entered<'c, 'db> {
    pub(super) fn new(cx: &'c Cx<'db>, timer: Timer) -> Self {
        Self { cx, timer: Some(timer) }
    }

    /// Ends the computation, returning what it read and whether its value stands.
    pub(super) fn leave(mut self) -> (Box<[Dep]>, Standing) {
        drop(self.timer.take());
        let left = self.cx.leave();
        std::mem::forget(self);
        left
    }
}

impl Drop for Entered<'_, '_> {
    /// A computation unwound: its frame goes, what it read with it.
    fn drop(&mut self) {
        let _ = self.cx.leave();
    }
}

/// A memo being verified on this thread.
pub(super) struct Verifying<'c, 'db> {
    cx: &'c Cx<'db>,
    at: (TableId, Slot),
}

impl<'c, 'db> Verifying<'c, 'db> {
    pub(super) fn start(cx: &'c Cx<'db>, at: (TableId, Slot)) -> Self {
        cx.verifying.borrow_mut().insert(at);
        Self { cx, at }
    }
}

impl Drop for Verifying<'_, '_> {
    fn drop(&mut self) {
        self.cx.verifying.borrow_mut().remove(&self.at);
    }
}
