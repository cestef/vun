//! A panic of the compiler, caught where it is part of the answer (`vun debug reduce`, trials of
//! programs made by chance), and where in the compiler's source it happened.

use std::cell::RefCell;
use std::fmt;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};

/// A place in the compiler's source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub file: String,
    pub line: u32,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)
    }
}

thread_local! {
    /// Where the last panic of this thread was, as the hook installed while catching records it.
    static PANICKED: RefCell<Option<Site>> = const { RefCell::new(None) };
}

/// A panic hook, as `std::panic` holds it.
type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send>;

/// The panic hook while panics are caught: one is recorded, not printed. The hook it replaced is
/// restored when this is dropped.
pub struct Catching(Option<Hook>);

impl Catching {
    #[must_use]
    pub fn install() -> Self {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(|info| {
            let site = info.location().map(|at| Site { file: at.file().to_owned(), line: at.line() });
            PANICKED.with_borrow_mut(|held| *held = site);
        }));
        Self(Some(previous))
    }

    /// What `run` gives, or where it panicked.
    ///
    /// # Errors
    /// Returns the place of the panic: the hook of `Catching` was installed.
    pub fn run<T>(&self, run: impl FnOnce() -> T) -> Result<T, Site> {
        panic::catch_unwind(AssertUnwindSafe(run)).map_err(|_| {
            PANICKED.with_borrow_mut(Option::take).unwrap_or(Site { file: "(unknown)".to_owned(), line: 0 })
        })
    }
}

impl Drop for Catching {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            panic::set_hook(previous);
        }
    }
}
