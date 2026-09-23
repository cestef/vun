//! High-level IR: the CST lowered into roles, names resolved within a file, and the checks that
//! need no types (spec 13, section 1).

pub mod dump;
pub mod lower;
pub mod resolve;
pub mod tree;
pub mod verify;
pub mod visit;
