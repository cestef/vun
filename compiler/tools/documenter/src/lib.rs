//! Documenters as components (decision 498): a [`Documenter`] writes the documentation of
//! packages (spec 12, section 7), as `vun doc` asks. [`Documenters`] is the interface a
//! documenter library is served through, [`Remote`] the `Documenter` the host calls it by. The
//! documenter is the same crate linked in or loaded; what only the compiler knows, whether a
//! declaration is a type, it asks the host.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use vun_codec::{Decode, Encode};
use vun_component::Cause;

pub use protocol::{Answer, Ask, Call, Reply};
pub use remote::Remote;
pub use serve::Documenters;

/// A namespace to document: its path, the file it is read from as the caller names it, and its
/// text.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Source {
    pub path: Vec<String>,
    pub file: String,
    pub text: String,
}

/// A file of the documentation: its path relative to the site's root, and its contents.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Written {
    pub path: String,
    pub contents: String,
}

/// The documentation of some namespaces.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Site {
    pub namespaces: usize,
    pub files: Vec<Written>,
}

/// What the compiler knows of the declarations of a namespace, which the text does not say.
pub trait Types {
    /// Whether the declaration `name` of the namespace at `path` declares a type.
    fn is_type(&self, path: &[String], name: &str) -> bool;
}

pub trait Documenter: Send + Sync {
    /// What `vun` calls it: `html`.
    fn name(&self) -> &'static str;

    /// The documentation of `sources`.
    ///
    /// # Errors
    /// Returns why there is none.
    fn document(&self, sources: &[Source], types: &dyn Types) -> Result<Site, Cause>;
}
