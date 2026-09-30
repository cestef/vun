//! The machine the toolchain runs on, named as targets name theirs: a build is for the first of
//! `targets.hosts` of its architecture and system unless told otherwise.

use crate::Name;

/// An architecture and a system, as a target's `arch` and `os` name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Machine {
    pub arch: Name,
    pub os: Name,
}

impl Machine {
    /// The machine this toolchain was built for.
    pub const RUNNING: Self = Self { arch: ARCH, os: Name(std::env::consts::OS) };
}

/// Rust names architectures as targets do, but for 64-bit ARM (`aarch64`, `arm64` here).
const ARCH: Name = if cfg!(target_arch = "aarch64") { Name("arm64") } else { Name(std::env::consts::ARCH) };
