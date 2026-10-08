//! Profiles written by callgrind: exact instruction counts, the same on every run of a
//! single-threaded program, whatever the machine.

mod cycles;
mod fold;
mod parse;
mod profile;
mod source;

#[cfg(test)]
mod tests;

pub use parse::{ParseError, Reason};
pub use profile::{Edge, Function, Profile, Share};
pub use source::Source;
