//! The numbers of the encoding: sections, value and import kinds, and the tool conventions'
//! relocations and symbol table, each declared once with its code.

/// What every module starts with, then its [`VERSION`].
pub const MAGIC: &[u8] = b"\0asm";
pub const VERSION: u32 = 1;

/// The bytes of a page of linear memory.
pub const PAGE: u64 = 65536;

/// The import module of what another object or the linker defines.
pub const ENV: &str = "env";

/// A section, by its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Custom = 0,
    Type = 1,
    Import = 2,
    Function = 3,
    Table = 4,
    Memory = 5,
    Global = 6,
    Export = 7,
    Start = 8,
    Element = 9,
    Code = 10,
    Data = 11,
    DataCount = 12,
}

impl Section {
    pub const ALL: [Self; 13] = [
        Self::Custom,
        Self::Type,
        Self::Import,
        Self::Function,
        Self::Table,
        Self::Memory,
        Self::Global,
        Self::Export,
        Self::Start,
        Self::Element,
        Self::Code,
        Self::Data,
        Self::DataCount,
    ];

    #[must_use]
    pub fn of(id: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|s| *s as u8 == id)
    }
}

/// A value type, by its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValType {
    I32 = 0x7f,
    I64 = 0x7e,
    F32 = 0x7d,
    F64 = 0x7c,
    V128 = 0x7b,
}

impl ValType {
    pub const ALL: [Self; 5] = [Self::I32, Self::I64, Self::F32, Self::F64, Self::V128];

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn of(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.code() == code)
    }
}

impl std::fmt::Display for ValType {
    /// As the text format spells it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::V128 => "v128",
        })
    }
}

/// A function type: what it takes and gives.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

/// What starts a function type.
pub const FUNC_TYPE: u8 = 0x60;
/// The reference type of tables of functions.
pub const FUNCREF: u8 = 0x70;
/// A global that can be set.
pub const MUTABLE: u8 = 1;

/// Sizes of a memory, in pages, or of a table, in entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub min: u32,
    pub max: Option<u32>,
}

/// What a module imports, by the kind's code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Import {
    /// A function of the type at this index.
    Func(u32),
    Table(Limits),
    Memory(Limits),
    Global {
        ty: ValType,
        mutable: bool,
    },
}

/// The kind of what a module imports or exports, by its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum External {
    Func = 0,
    Table = 1,
    Memory = 2,
    Global = 3,
}

impl External {
    pub const ALL: [Self; 4] = [Self::Func, Self::Table, Self::Memory, Self::Global];

    #[must_use]
    pub fn of(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == code)
    }
}

/// A relocation's kind, by its number in the tool conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocKind {
    FunctionIndexLeb = 0,
    TableIndexSleb = 1,
    TableIndexI32 = 2,
    MemoryAddrLeb = 3,
    MemoryAddrSleb = 4,
    MemoryAddrI32 = 5,
    TypeIndexLeb = 6,
    GlobalIndexLeb = 7,
    TableNumberLeb = 20,
}

impl RelocKind {
    pub const ALL: [Self; 9] = [
        Self::FunctionIndexLeb,
        Self::TableIndexSleb,
        Self::TableIndexI32,
        Self::MemoryAddrLeb,
        Self::MemoryAddrSleb,
        Self::MemoryAddrI32,
        Self::TypeIndexLeb,
        Self::GlobalIndexLeb,
        Self::TableNumberLeb,
    ];

    #[must_use]
    pub fn of(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == code)
    }

    /// Whether the entry carries an addend: the kinds of memory addresses.
    #[must_use]
    pub const fn has_addend(self) -> bool {
        matches!(self, Self::MemoryAddrLeb | Self::MemoryAddrSleb | Self::MemoryAddrI32)
    }
}

/// A symbol's kind in the `linking` section's symbol table, by its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function = 0,
    Data = 1,
    Global = 2,
    Section = 3,
    Tag = 4,
    Table = 5,
}

impl SymbolKind {
    pub const ALL: [Self; 6] =
        [Self::Function, Self::Data, Self::Global, Self::Section, Self::Tag, Self::Table];

    #[must_use]
    pub fn of(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == code)
    }
}

/// A symbol's flags in the symbol table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolFlags(pub u32);

impl SymbolFlags {
    pub const WEAK: Self = Self(0x1);
    pub const LOCAL: Self = Self(0x2);
    pub const HIDDEN: Self = Self(0x4);
    pub const UNDEFINED: Self = Self(0x10);
    pub const EXPORTED: Self = Self(0x20);
    pub const EXPLICIT_NAME: Self = Self(0x40);
    pub const NO_STRIP: Self = Self(0x80);

    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    #[must_use]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }
}

/// The version of the `linking` section, and its subsections.
pub const LINKING_VERSION: u32 = 2;
pub const SEGMENT_INFO: u8 = 5;
pub const INIT_FUNCS: u8 = 6;
pub const COMDAT_INFO: u8 = 7;
pub const SYMBOL_TABLE: u8 = 8;

/// A segment's flags in `linking`'s segment info: its bytes are strings the linker may merge,
/// or storage per thread.
pub const SEGMENT_STRINGS: u32 = 0x1;
pub const SEGMENT_TLS: u32 = 0x2;

/// What a member of a COMDAT group is, by its code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComdatKind {
    Data = 0,
    Function = 1,
    Global = 2,
    Tag = 3,
    Section = 4,
    Table = 5,
}

impl ComdatKind {
    pub const ALL: [Self; 6] =
        [Self::Data, Self::Function, Self::Global, Self::Tag, Self::Section, Self::Table];

    #[must_use]
    pub fn of(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == code)
    }
}

/// The names of the custom sections the tool conventions and the name section use.
pub const LINKING: &str = "linking";
pub const RELOC: &str = "reloc.";
pub const NAME: &str = "name";
/// The name section's subsection naming functions.
pub const FUNCTION_NAMES: u8 = 1;

/// The instructions of constant expressions, which data segments and globals start from, and
/// those of the functions a linker makes.
pub mod op {
    pub const UNREACHABLE: u8 = 0x00;
    pub const END: u8 = 0x0b;
    pub const CALL: u8 = 0x10;
    pub const I32_CONST: u8 = 0x41;
}
