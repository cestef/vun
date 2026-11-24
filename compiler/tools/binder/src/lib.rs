//! Binders as components (decision 498): a [`Binder`] reads a foreign header into Vun
//! declarations (spec 07, section 5), as `vun util ffi bind` asks. [`Binders`] is the interface a
//! binder library is served through, [`Remote`] the `Binder` the host calls it by. The binder is
//! the same crate linked in or loaded, and asks nothing of the host.

mod bindings;
mod error;
mod glob;
mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use vun_codec::{Decode, Encode};

pub use bindings::{
    Bindings, Bytes, Failure, Library, Missing, Part, Reason, Skipped, Unavailable, Unsupported,
};
pub use error::BindError;
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
    /// The enums, by the names they are bound under, that are Vun enums rather than integers: those
    /// every value of which C names and never goes beyond.
    pub closed: Vec<Glob>,
    /// The libraries the bound functions and variables are linked from (`#[link.library(name)]`).
    pub libraries: Vec<Library>,
    /// The namespace the bindings are written as, such as `app.sys`, when each directory of
    /// headers is bound in a file of its own under it: they name each other by it.
    pub module: Option<Vec<String>>,
}

impl Request {
    /// A request to bind all of `header` with the compile flags `args`, avoiding `reserved`.
    #[must_use]
    pub fn new(header: PathBuf, args: Vec<String>, reserved: Vec<String>) -> Self {
        Self {
            header,
            args,
            reserved,
            scope: Vec::new(),
            only: Vec::new(),
            closed: Vec::new(),
            libraries: Vec::new(),
            module: None,
        }
    }
}

pub trait Binder: Send + Sync {
    /// What `vun` calls it: `c`.
    fn name(&self) -> &'static str;

    /// The bindings of the header.
    ///
    /// # Errors
    /// Returns why nothing could be bound.
    fn bind(&self, request: &Request) -> Result<Bindings, BindError>;
}
