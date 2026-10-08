//! Filesystem operations shared by the compiler and its tests: walks in a stable order, so that
//! everything built from a walk (hashes, corpus runs, snapshots) is deterministic, writes that
//! create what they need, directories replaced whole, and the executable bit.

mod captured;
mod edit;
mod executable;
mod mapped;
mod mode;
mod mounts;
mod process;
mod refusal;
mod relative;
mod scratch;
mod walk;

pub use captured::{Captured, Ended};
pub use edit::{Edit, Part};
pub use executable::EXECUTABLE;
pub use mapped::Mapped;
pub use mode::{Mode, Modes};
pub use mounts::Mounts;
pub use process::PROCESS;
pub use refusal::Refusal;
pub use relative::Relative;
pub use scratch::Scratch;
pub use walk::{Tree, WORK};

#[cfg(test)]
mod tests;
