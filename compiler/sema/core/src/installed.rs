//! What a crate above sema provides to it: sema defines the interface, the driver installs the
//! implementation in the query database once, and sema calls it without depending on it.

use std::sync::{Arc, OnceLock};

/// The installed implementation of the interface `T`: a resource of the query database.
pub struct Installed<T: ?Sized>(OnceLock<Arc<T>>);

impl<T: ?Sized> Default for Installed<T> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

impl<T: ?Sized> Installed<T> {
    /// Installs `implementation`, unless one is installed.
    pub fn install(&self, implementation: Arc<T>) {
        let _ = self.0.set(implementation);
    }

    /// The implementation installed, if any.
    #[must_use]
    pub fn get(&self) -> Option<&Arc<T>> {
        self.0.get()
    }
}
