//! A target's C: the types its C compiler lays out, and the compiler itself.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

/// `lang.target.CInt`: a C integer type, its width in bits and alignment in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CInt {
    pub bits: u32,
    pub align: u32,
}

/// `lang.target.BuildsFor`: the machine a C compiler's command builds for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum BuildsFor {
    Target,
    /// The machine it runs on, as a system's own `cc` does.
    Host,
}

/// `lang.target.Atomics`: what a C compiler offers beyond C99 for atomic access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Atomics {
    None,
    /// The `__atomic_*` builtins of GCC and Clang.
    Gnu,
}

/// `lang.target.Attributes`: what a C compiler offers beyond C99 for placing data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Attributes {
    None,
    /// `__attribute__((aligned(n), section(name)))` of GCC and Clang.
    Gnu,
}

/// `lang.target.FrameSizes`: how a C compiler reports frame sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum FrameSizes {
    None,
    /// `-fstack-usage`, a `.su` file beside each object.
    StackUsage,
}

/// `lang.target.CCompiler`: the C compiler building a target's C, and driving its link.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CCompiler {
    pub command: Symbol,
    pub builds_for: BuildsFor,
    /// The program bundling objects into a static library.
    pub archiver: Symbol,
    pub flags: Vec<Symbol>,
    pub link_flags: Vec<Symbol>,
    /// The libraries its links take after the objects, found where it finds its own.
    pub libraries: Vec<Symbol>,
    /// The flags a release build adds to `flags`.
    pub optimize: Vec<Symbol>,
    pub atomics: Atomics,
    pub attributes: Attributes,
    pub frame_sizes: FrameSizes,
    /// The option telling it the oldest release of the target's system.
    pub version_flag: Option<Symbol>,
    /// The directory of the target system's headers and libraries its tools build against.
    pub sysroot: Option<Symbol>,
}

/// `lang.target.C`: the C types of a target, and its C compiler.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CTypes {
    pub short: CInt,
    pub int: CInt,
    pub long: CInt,
    pub long_long: CInt,
    pub pointer_align: u32,
    pub compiler: CCompiler,
}
