//! Filesystem operations shared by the compiler and its tests: walks in a stable order, so that
//! everything built from a walk (hashes, corpus runs, snapshots) is deterministic, writes that
//! create what they need, and directories replaced whole.

mod edit;
mod scratch;
mod walk;

pub use edit::Edit;
pub use scratch::Scratch;
pub use walk::Tree;

#[cfg(test)]
mod tests;
