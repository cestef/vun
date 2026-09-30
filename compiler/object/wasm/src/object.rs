//! A relocatable object as the linker reads it: its types, imports and functions, the bytes of
//! its code and data as written, where each body and segment lies in them, its symbols and the
//! relocations of its code and data.

use std::ops::Range;

use vun_bytes::Out;

use crate::format::{FuncType, Import, RelocKind, SymbolFlags};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Object {
    pub types: Vec<FuncType>,
    pub imports: Vec<ObjectImport>,
    /// The type of each function it defines, in the order of their indices past the imported.
    pub funcs: Vec<u32>,
    pub code: Code,
    pub data: Data,
    pub symbols: Vec<Symbol>,
    /// The name each function it exports under another name than its symbol's has
    /// (`export_name`), by its index in the function index space.
    pub exports: Vec<(String, u32)>,
    /// Functions to call before the program starts, by symbol, lowest priority first as read.
    pub inits: Vec<Init>,
    pub comdats: Vec<Comdat>,
}

/// A function called before the program starts: the lower its priority, the sooner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Init {
    pub priority: u32,
    pub symbol: u32,
}

/// A COMDAT group: functions and segments the first object declaring it keeps, and every later
/// one drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comdat {
    pub name: String,
    /// The defined functions it holds, by index past the imported ones.
    pub funcs: Vec<u32>,
    pub segments: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectImport {
    pub module: String,
    pub name: String,
    pub import: Import,
}

/// The code section's payload, where each body lies in it after its size, and the relocations
/// of the payload, by offset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Code {
    pub payload: Out,
    pub bodies: Vec<Range<usize>>,
    pub relocs: Vec<Reloc>,
}

/// The data section's payload, its segments, and the relocations of the payload, by offset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Data {
    pub payload: Out,
    pub segments: Vec<Segment>,
    pub relocs: Vec<Reloc>,
}

/// A data segment: where its bytes lie in the payload, and the name and alignment `linking`
/// gives it; its name is the section it goes in (`.rodata.x`, `.data.x`, `.bss.x`, or one the
/// source names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub bytes: Range<usize>,
    pub name: String,
    pub align: u64,
}

/// A relocation of a payload: at an offset in it, of a symbol's index (a type's for
/// [`RelocKind::TypeIndexLeb`]), plus an addend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reloc {
    pub kind: RelocKind,
    pub offset: usize,
    pub index: u32,
    pub addend: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub flags: SymbolFlags,
    /// Its name: the one it is given, or an undefined one's import's; none for a local left
    /// unnamed.
    pub name: Option<String>,
    pub what: Symbolic,
}

impl Symbol {
    #[must_use]
    pub const fn defined(&self) -> bool {
        !self.flags.has(SymbolFlags::UNDEFINED)
    }

    #[must_use]
    pub const fn local(&self) -> bool {
        self.flags.has(SymbolFlags::LOCAL)
    }
}

/// What a symbol names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbolic {
    /// The function at this index of the object's function index space: an import's below the
    /// number of imported functions, a defined one's past it.
    Function(u32),
    /// Bytes of a segment, when defined here.
    Data(Option<DataDef>),
    Global(u32),
    Table(u32),
    /// Another kind the linker does not relocate against (sections, tags).
    Other(u8),
}

/// Where a data symbol's bytes are: a segment, an offset into it, and how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataDef {
    pub segment: u32,
    pub offset: u32,
    pub size: u32,
}

impl Object {
    /// The imported functions, in the order of their indices.
    pub fn imported_funcs(&self) -> impl Iterator<Item = (&ObjectImport, u32)> {
        self.imports.iter().filter_map(|import| match import.import {
            Import::Func(ty) => Some((import, ty)),
            _ => None,
        })
    }
}
