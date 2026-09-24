//! Target descriptions: layout, ABIs, features, entry symbol.
//!
//! Targets are data. The canonical descriptions are Vun values in `lib/targets`; this crate is
//! their Rust mirror, filled in by evaluating those values.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectFormat {
    Elf,
    MachO,
    Coff,
    Wasm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Abi {
    Native,
    C,
    Interrupt,
    Naked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetSpec {
    /// Binding path in the `targets` package, such as `thumbv7em.none_eabihf`.
    pub name: String,
    pub arch: String,
    pub pointer_bits: u8,
    pub endian: Endian,
    pub native_int_bits: u8,
    pub max_align: u8,
    pub abis: Vec<Abi>,
    pub features: Vec<String>,
    pub object_format: ObjectFormat,
    /// The linker's entry symbol; `None` for libraries (spec 08, rule 2).
    pub entry_symbol: Option<String>,
    pub default_backend: String,
    pub default_linker: String,
}

impl TargetSpec {
    #[must_use]
    pub fn usize_max(&self) -> u128 {
        (1u128 << self.pointer_bits) - 1
    }
}

/// The architectures a component handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arches {
    Any,
    Only(&'static [&'static str]),
}

impl Arches {
    #[must_use]
    pub fn contains(self, arch: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Only(list) => list.contains(&arch),
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
    Arch(String),
    ObjectFormat(ObjectFormat),
    MemoryRegions,
}
