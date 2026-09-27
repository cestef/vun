//! The tool conventions' linking metadata: relocation kinds, symbol flags, the `linking` section
//! (symbol table, segment info) and the `reloc.*` sections.

use vun_bytes::Out;
use vun_index::Idx;

use super::{Binding, Def, Kind, Linker, Module, SymbolId};

/// The version of the `linking` section.
const VERSION: u64 = 2;

/// Subsections of `linking`.
const SEGMENT_INFO: u8 = 5;
const SYMBOL_TABLE: u8 = 8;

/// A relocation's kind, by its number in the conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RelocKind {
    FunctionIndexLeb = 0,
    TableIndexSleb = 1,
    TableIndexI32 = 2,
    MemoryAddrSleb = 4,
    MemoryAddrI32 = 5,
    TypeIndexLeb = 6,
    GlobalIndexLeb = 7,
    TableNumberLeb = 20,
}

impl RelocKind {
    /// Whether the entry carries an addend: the kinds of memory addresses.
    fn has_addend(self) -> bool {
        matches!(self, Self::MemoryAddrSleb | Self::MemoryAddrI32)
    }
}

/// A relocation of a section's bytes: at an offset of its payload, of a symbol's (or, for a type
/// index, a type's) index, plus an addend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Reloc {
    pub kind: RelocKind,
    pub offset: u32,
    pub index: u32,
    pub addend: i32,
}

/// Symbol flags.
const LOCAL: u64 = 0x2;
const HIDDEN: u64 = 0x4;
const UNDEFINED: u64 = 0x10;
const EXPORTED: u64 = 0x20;
const EXPLICIT_NAME: u64 = 0x40;
const NO_STRIP: u64 = 0x80;

/// Symbol kinds.
const FUNCTION: u8 = 0;
const DATA: u8 = 1;
const GLOBAL: u8 = 2;
const TABLE: u8 = 5;

impl Module {
    /// The `linking` section's payload: the symbol table, then each segment's name and alignment.
    pub(super) fn linking(&self, functions: &[SymbolId]) -> Out {
        let mut out = Out::default();
        out.uleb(VERSION);
        let mut table = Out::default();
        table.uleb(self.symbols.len() as u64);
        for (id, sym) in self.symbols.iter() {
            let binding = match sym.binding {
                Binding::Local => LOCAL,
                Binding::Hidden => HIDDEN,
                Binding::Exported => EXPORTED,
            };
            let name = sym.name.as_str().as_bytes();
            match &sym.kind {
                Kind::Func { def, .. } => {
                    let index = functions.iter().position(|&f| f == id).expect("every function is indexed");
                    let undefined = matches!(def, Def::Imported { .. });
                    let flags = if undefined { UNDEFINED | EXPLICIT_NAME } else { binding };
                    table.u8(FUNCTION);
                    table.uleb(flags);
                    table.uleb(index as u64);
                    table.name(name);
                }
                Kind::Data(segment) => {
                    table.u8(DATA);
                    table.uleb(if segment.is_some() { binding } else { UNDEFINED });
                    table.name(name);
                    if let Some(at) = segment {
                        table.uleb(*at as u64);
                        table.uleb(0);
                        table.uleb(self.segments[*at].bytes.len() as u64);
                    }
                }
                Kind::Linker(Linker::StackPointer) => {
                    table.u8(GLOBAL);
                    table.uleb(UNDEFINED);
                    table.uleb(0);
                }
                Kind::Linker(Linker::Table) => {
                    table.u8(TABLE);
                    table.uleb(UNDEFINED | NO_STRIP);
                    table.uleb(0);
                }
            }
        }
        out.subsection(SYMBOL_TABLE, &table);
        if !self.segments.is_empty() {
            let mut info = Out::default();
            info.uleb(self.segments.len() as u64);
            for (at, segment) in self.segments.iter().enumerate() {
                let prefix: &[u8] = if segment.writable { b".data." } else { b".rodata." };
                info.name(&[prefix, at.to_string().as_bytes()].concat());
                info.uleb(u64::from(segment.align.trailing_zeros()));
                info.uleb(0);
            }
            out.subsection(SEGMENT_INFO, &info);
        }
        out
    }
}

/// A `reloc.*` section's payload: the relocations of the section at `section`, by offset.
pub(super) fn relocs(section: u32, relocs: &[Reloc]) -> Out {
    let mut out = Out::default();
    out.uleb(u64::from(section));
    out.uleb(relocs.len() as u64);
    for r in relocs {
        out.u8(r.kind as u8);
        out.uleb(u64::from(r.offset));
        out.uleb(u64::from(r.index));
        if r.kind.has_addend() {
            out.sleb(i64::from(r.addend));
        }
    }
    out
}

/// The encodings of wasm: names as their length and bytes, subsections as an id and a length.
pub(super) trait Wasm {
    fn name(&mut self, name: &[u8]);

    fn subsection(&mut self, id: u8, payload: &Out);
}

impl Wasm for Out {
    fn name(&mut self, name: &[u8]) {
        self.uleb(name.len() as u64);
        self.bytes(name);
    }

    fn subsection(&mut self, id: u8, payload: &Out) {
        self.u8(id);
        self.uleb(payload.here());
        self.bytes(&payload.0);
    }
}

impl SymbolId {
    pub(super) fn number(self) -> u32 {
        u32::try_from(self.index()).expect("symbols fit u32")
    }
}
