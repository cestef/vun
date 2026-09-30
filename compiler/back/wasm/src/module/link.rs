//! The tool conventions' linking metadata: relocation kinds, symbol flags, the `linking` section
//! (symbol table, segment info) and the `reloc.*` sections.

use vun_bytes::Out;
use vun_index::Idx;
pub(super) use vun_wasm::RelocKind;
use vun_wasm::format::{LINKING_VERSION, SEGMENT_INFO, SYMBOL_TABLE};
use vun_wasm::{SymbolFlags, SymbolKind, Wasm};

use super::{Binding, Def, Kind, Linker, Module, SymbolId};

/// A relocation of a section's bytes: at an offset of its payload, of a symbol's (or, for a type
/// index, a type's) index, plus an addend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Reloc {
    pub kind: RelocKind,
    pub offset: u32,
    pub index: u32,
    pub addend: i32,
}

impl Module {
    /// The `linking` section's payload: the symbol table, then each segment's name and alignment.
    pub(super) fn linking(&self, functions: &[SymbolId]) -> Out {
        let mut out = Out::default();
        out.uleb(u64::from(LINKING_VERSION));
        let mut table = Out::default();
        table.uleb(self.symbols.len() as u64);
        for (id, sym) in self.symbols.iter() {
            let binding = match sym.binding {
                Binding::Local => SymbolFlags::LOCAL,
                Binding::Hidden => SymbolFlags::HIDDEN,
                Binding::Exported => SymbolFlags::EXPORTED,
            };
            let name = sym.name.as_str().as_bytes();
            match &sym.kind {
                Kind::Func { def, .. } => {
                    let index = functions.iter().position(|&f| f == id).expect("every function is indexed");
                    let undefined = matches!(def, Def::Imported { .. });
                    let flags = if undefined {
                        SymbolFlags::UNDEFINED.with(SymbolFlags::EXPLICIT_NAME)
                    } else {
                        binding
                    };
                    table.u8(SymbolKind::Function as u8);
                    table.uleb(u64::from(flags.0));
                    table.uleb(index as u64);
                    table.name(name);
                }
                Kind::Data(segment) => {
                    table.u8(SymbolKind::Data as u8);
                    table.uleb(u64::from(if segment.is_some() { binding } else { SymbolFlags::UNDEFINED }.0));
                    table.name(name);
                    if let Some(at) = segment {
                        table.uleb(*at as u64);
                        table.uleb(0);
                        table.uleb(self.segments[*at].bytes.len() as u64);
                    }
                }
                Kind::Linker(Linker::StackPointer) => {
                    table.u8(SymbolKind::Global as u8);
                    table.uleb(u64::from(SymbolFlags::UNDEFINED.0));
                    table.uleb(0);
                }
                Kind::Linker(Linker::Table) => {
                    table.u8(SymbolKind::Table as u8);
                    table.uleb(u64::from(SymbolFlags::UNDEFINED.with(SymbolFlags::NO_STRIP).0));
                    table.uleb(0);
                }
            }
        }
        out.subsection(SYMBOL_TABLE, &table);
        if !self.segments.is_empty() {
            let mut info = Out::default();
            info.uleb(self.segments.len() as u64);
            for (at, segment) in self.segments.iter().enumerate() {
                if let Some(section) = segment.section {
                    info.name(section.as_str().as_bytes());
                } else {
                    let prefix: &[u8] = if segment.writable { b".data." } else { b".rodata." };
                    info.name(&[prefix, at.to_string().as_bytes()].concat());
                }
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
    out.vector(relocs.iter(), |out, r| {
        out.u8(r.kind as u8);
        out.uleb(u64::from(r.offset));
        out.uleb(u64::from(r.index));
        if r.kind.has_addend() {
            out.sleb(i64::from(r.addend));
        }
    });
    out
}

impl SymbolId {
    pub(super) fn number(self) -> u32 {
        u32::try_from(self.index()).expect("symbols fit u32")
    }
}
