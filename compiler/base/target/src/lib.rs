//! Targets as the compiler reads them: `lang.target.Spec` values of the `targets` package (spec
//! 13, section 7), and the names components declare what they handle by.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

/// A name of one of the open sets targets draw from (decision 138): architectures, systems,
/// object formats, backends and linkers. Targets give theirs as `lang.meta.Name` values;
/// components declare theirs statically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(pub &'static str);

/// A name persists as a symbol, whose text lives as long as the process.
impl Encode for Name {
    fn encode(&self, w: &mut vun_codec::Writer<'_>) {
        vun_intern::Symbol::intern(self.0).encode(w);
    }
}

impl Decode for Name {
    fn decode(r: &mut vun_codec::Reader<'_>) -> Result<Self, vun_codec::Malformed> {
        vun_intern::Symbol::decode(r).map(|name| Self(name.as_str()))
    }
}

impl Name {
    /// Whether a target's name is this one.
    #[must_use]
    pub fn is(self, name: Symbol) -> bool {
        name.as_str() == self.0
    }

    #[must_use]
    pub fn symbol(self) -> Symbol {
        Symbol::intern(self.0)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// `lang.target.Endian`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Endian {
    Little,
    Big,
}

/// `lang.target.CInt`: a C integer type, its width in bits and alignment in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CInt {
    pub bits: u32,
    pub align: u32,
}

/// `lang.target.Atomics`: what a C compiler offers beyond C99 for atomic access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Atomics {
    None,
    /// The `__atomic_*` builtins of GCC and Clang.
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
    pub flags: Vec<Symbol>,
    pub link_flags: Vec<Symbol>,
    pub atomics: Atomics,
    pub frame_sizes: FrameSizes,
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

/// `lang.target.Spec`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct TargetSpec {
    pub arch: Symbol,
    pub cpu: Symbol,
    pub os: Symbol,
    pub abi: Symbol,
    pub pointer_bits: u32,
    pub endian: Endian,
    pub object_format: Symbol,
    /// The linker's entry symbol; `None` for libraries (spec 08, rule 2).
    pub entry_symbol: Option<Symbol>,
    pub default_backend: Symbol,
    pub default_linker: Symbol,
    pub c: CTypes,
}

/// `lang.target.Host`: a machine the toolchain runs on, named as the toolchain names it (Rust's
/// `std::env::consts`), and the target it builds for there by default.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Host {
    pub arch: Symbol,
    pub os: Symbol,
    pub target: TargetSpec,
}

/// The architectures a component handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arches {
    Any,
    Only(&'static [Name]),
}

impl Arches {
    #[must_use]
    pub fn contains(self, arch: Symbol) -> bool {
        match self {
            Self::Any => true,
            Self::Only(list) => list.iter().any(|n| n.is(arch)),
        }
    }
}

/// Whether a component supports a target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Support {
    Yes,
    No(Unsupported),
}

/// Why a component cannot handle a target or request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Arch(Symbol),
    ObjectFormat(Symbol),
    MemoryRegions,
}
