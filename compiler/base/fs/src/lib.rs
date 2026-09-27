//! Filesystem operations shared by the compiler and its tests: walks in a stable order, so that
//! everything built from a walk (hashes, corpus runs, snapshots) is deterministic, writes that
//! create what they need, directories replaced whole, and the executable bit.

mod edit;
mod mode;
mod refusal;
mod relative;
mod scratch;
mod walk;

pub use edit::Edit;
pub use mode::{Mode, Modes};
pub use refusal::Refusal;
pub use relative::Relative;
pub use scratch::Scratch;
pub use walk::{Tree, WORK};

#[cfg(test)]
mod tests;
