//! Binders as components (decision 498): a [`Binder`] reads a foreign header into Vun
//! declarations (spec 07, section 5), as `vun util ffi bind` asks. [`Binders`] is the interface a
//! binder library is served through, [`Remote`] the `Binder` the host calls it by. The binder is
//! the same crate linked in or loaded, and asks nothing of the host.

mod bindings;
mod glob;
mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use vun_codec::{Decode, Encode};

pub use bindings::{Bindings, Bytes, Failure, Library, Missing, Reason, Skipped, Unavailable, Unsupported};
pub use glob::Glob;
pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Binders;

/// What is bound: a header, as the build compiles it.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Request {
    pub header: PathBuf,
    /// The compile flags of the build, such as `-Iinclude` and `--target`.
    pub args: Vec<String>,
    /// The names the file the bindings are written to sees already, which they avoid.
    pub reserved: Vec<String>,
    /// The directories whose declarations are bound with those of the header's own: a header that
    /// includes others from elsewhere, as GTK includes `GLib`'s, binds them by naming where they are.
    pub scope: Vec<PathBuf>,
    /// What is bound: the declarations one of these names, and those they need. Every declaration
    /// in scope when there is none.
    pub only: Vec<Glob>,
    /// The libraries the bound functions and variables are linked from (`#[link.library(name)]`).
    pub libraries: Vec<Library>,
}

impl Request {
    /// A request to bind all of `header` with the compile flags `args`, avoiding `reserved`.
    #[must_use]
    pub fn new(header: PathBuf, args: Vec<String>, reserved: Vec<String>) -> Self {
        Self { header, args, reserved, scope: Vec::new(), only: Vec::new(), libraries: Vec::new() }
    }
}

pub trait Binder: Send + Sync {
    /// What `vun` calls it: `c`.
    fn name(&self) -> &'static str;

    /// The bindings of the header.
    ///
    /// # Errors
    /// Returns why nothing could be bound.
    fn bind(&self, request: &Request) -> Result<Bindings, Failure>;
}
