//! Progress of work that takes a while, drawn on a terminal's error stream: a bar per task, under
//! the lines the command writes. Each [`Bar`] is a handle any thread may advance; a thread of
//! the [`Progress`] redraws what changed a few times a second, and the stream clears the bars
//! before it writes a line and draws them again after it. Where nothing is drawn (a pipe,
//! `--format json`, `--quiet`), bars cost an atomic add.

mod bar;
mod draw;
#[cfg(test)]
mod tests;

use std::io::Write;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, Weak};
use std::thread;
use std::time::Duration;

pub use bar::{Bar, Live, Unit};

use crate::{Color, Text, Verb};
use bar::Task;

/// The width of a terminal when it is not known.
pub const DEFAULT_WIDTH: usize = 80;

/// How often the bars are drawn again.
const REFRESH: Duration = Duration::from_millis(100);

/// The bars of a command, drawn or not. Clones share them.
#[derive(Clone, Default)]
pub struct Progress(Option<Arc<Mutex<State>>>);

/// The bars off the terminal, and nothing drawn until it is dropped.
pub struct Held<'a>(MutexGuard<'a, State>);

impl Held<'_> {
    /// Notes whether what was written ended a line: bars are not drawn in the middle of one.
    pub fn ended_line(&mut self, ended: bool) {
        self.0.at_line_start = ended;
    }
}

/// What is drawn and what is to be: the tasks running, and how many lines of them are on the
/// terminal.
struct State {
    tasks: Vec<Arc<Task>>,
    shown: usize,
    /// Whether the stream is at the start of a line, where bars may be drawn.
    at_line_start: bool,
    sink: Box<dyn Write + Send>,
    color: Color,
    width: usize,
}

/// The progress of the command running, for code that reports work without being handed it, as
/// a download does: set once the command's output is known.
static AMBIENT: RwLock<Progress> = RwLock::new(Progress::off());

impl Progress {
    /// Makes this the progress [`Self::ambient`] gives.
    pub fn make_ambient(&self) {
        *AMBIENT.write().unwrap_or_else(PoisonError::into_inner) = self.clone();
    }

    /// The progress of the command running; draws nothing where there is none.
    #[must_use]
    pub fn ambient() -> Self {
        AMBIENT.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Progress that draws nothing.
    #[must_use]
    pub const fn off() -> Self {
        Self(None)
    }

    /// Progress drawn to `sink`, a terminal `width` columns wide, in `color`.
    #[must_use]
    pub fn drawn(sink: Box<dyn Write + Send>, color: Color, width: usize) -> Self {
        let state = Arc::new(Mutex::new(State {
            tasks: Vec::new(),
            shown: 0,
            at_line_start: true,
            sink,
            color,
            width,
        }));
        let weak = Arc::downgrade(&state);
        // Best effort: without a thread, bars are drawn when a line is written.
        let _ = thread::Builder::new().name("progress".into()).spawn(move || Self::refresh(&weak));
        Self(Some(state))
    }

    /// Whether bars are drawn.
    #[must_use]
    pub const fn is_drawn(&self) -> bool {
        self.0.is_some()
    }

    fn refresh(state: &Weak<Mutex<State>>) {
        while let Some(state) = state.upgrade() {
            Self::lock(&state).draw();
            drop(state);
            thread::sleep(REFRESH);
        }
    }

    fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
        state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A bar for `total` units of work (0 when not known), under the verb `verb` and the
    /// subject `subject`. It goes away when dropped.
    #[must_use]
    pub fn bar(&self, verb: Verb, subject: Text, total: u64, unit: Unit) -> Bar {
        let task = Arc::new(Task::new(verb, subject, total, unit));
        if let Some(state) = &self.0 {
            Self::lock(state).tasks.push(Arc::clone(&task));
        }
        Bar::new(self.clone(), task)
    }

    /// A line saying what `live` reports while it reports something, from `delay` after it is
    /// made: for a command to show what stage of its work runs, without counting it.
    #[must_use]
    pub fn watch(&self, verb: Verb, delay: Duration, live: Live) -> Bar {
        let mut task = Task::new(verb, Text::new(), 0, Unit::Bytes);
        task.live = Some(live);
        task.delay = delay;
        let task = Arc::new(task);
        if let Some(state) = &self.0 {
            Self::lock(state).tasks.push(Arc::clone(&task));
        }
        Bar::new(self.clone(), task)
    }

    /// Takes the bars off the terminal until the guard is dropped: for a stream about to write.
    #[must_use]
    pub fn hold(&self) -> Option<Held<'_>> {
        let mut state = Self::lock(self.0.as_deref()?);
        state.erase();
        Some(Held(state))
    }

    /// Ends every bar and takes them off the terminal: for a command that is done, or about to
    /// give the terminal to a program.
    pub fn clear(&self) {
        if let Some(state) = &self.0 {
            let mut state = Self::lock(state);
            state.tasks.clear();
            state.erase();
        }
    }

    /// Draws the bars again, after a line was written.
    pub fn redraw(&self) {
        if let Some(state) = &self.0 {
            Self::lock(state).draw();
        }
    }

    fn remove(&self, task: &Arc<Task>) {
        if let Some(state) = &self.0 {
            let mut state = Self::lock(state);
            state.tasks.retain(|t| !Arc::ptr_eq(t, task));
            state.draw();
        }
    }
}
