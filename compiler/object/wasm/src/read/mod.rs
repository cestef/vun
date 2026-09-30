//! Reading a relocatable object: its sections in order, then the `linking` section's symbols
//! and segment alignments, then the relocations of its code and data; the entries of each
//! section are read in `entries`.

mod entries;

use crate::format::{
    COMDAT_INFO, INIT_FUNCS, Import, LINKING, LINKING_VERSION, MAGIC, RELOC, RelocKind, SEGMENT_INFO,
    SEGMENT_TLS, SYMBOL_TABLE, Section, SymbolFlags, SymbolKind, VERSION,
};
use crate::input::{Coded, Input, Malformed};
use crate::object::{Code, Data, DataDef, Init, Object, Reloc, Symbol, Symbolic};

/// The bytes of the field each relocation kind rewrites.
const LEB: usize = vun_bytes::PADDED;
const I32: usize = 4;

impl Object {
    /// The object `bytes` hold.
    ///
    /// # Errors
    /// Where they are not a relocatable object in the tool conventions' format, or hold what
    /// the linker does not read.
    pub fn read(bytes: &[u8]) -> Result<Self, Malformed> {
        let mut input = Input::new(bytes);
        if input.take(MAGIC.len())? != MAGIC || input.u32()? != VERSION {
            return Err(Malformed::Magic);
        }
        let mut object = Self::default();
        let (mut code, mut data) = (None, None);
        let mut linking = None;
        let mut relocs = Vec::new();
        for number in 0.. {
            if input.done() {
                break;
            }
            let id = input.coded(Coded::Section, Section::of)?;
            let payload = input.sized()?;
            let mut section = Input::new(payload);
            match id {
                Section::Type => object.types = section.vector(Input::func_type)?,
                Section::Import => object.imports = section.vector(Input::import)?,
                Section::Function => object.funcs = section.vector(Input::uleb)?,
                Section::Code => {
                    object.code = Code::read(payload)?;
                    code = Some(number);
                }
                Section::Data => {
                    object.data = Data::read(payload)?;
                    data = Some(number);
                }
                // An object's table entries and count of segments are its own view: the linker builds
                // the table from the relocations taking functions' addresses, as `wasm-ld` does.
                Section::Element | Section::DataCount => {}
                Section::Export => {
                    object.exports = section.vector(Input::export)?.into_iter().flatten().collect();
                }
                Section::Custom => {
                    let name = section.name()?;
                    let rest = &payload[section.at..];
                    if name == LINKING {
                        linking = Some(rest);
                    } else if name.starts_with(RELOC) {
                        relocs.push(rest);
                    }
                }
                other => return Err(Malformed::Unsupported { what: Coded::Section, code: other as u32 }),
            }
        }
        object.linking(linking.ok_or(Malformed::Linking)?)?;
        for init in &object.inits {
            if init.symbol as usize >= object.symbols.len() {
                return Err(Malformed::Index { what: Coded::Symbol, index: init.symbol });
            }
        }
        for rest in relocs {
            let mut input = Input::new(rest);
            let section = input.uleb()?;
            let entries = input.vector(Input::reloc)?;
            if Some(section) == code {
                object.code.relocs = object.checked(entries, object.code.payload.0.len())?;
            } else if Some(section) == data {
                object.data.relocs = object.checked(entries, object.data.payload.0.len())?;
            }
        }
        Ok(object)
    }

    /// The symbol table and the segments' alignments.
    fn linking(&mut self, bytes: &[u8]) -> Result<(), Malformed> {
        let mut input = Input::new(bytes);
        if input.uleb()? != LINKING_VERSION {
            return Err(Malformed::Linking);
        }
        while !input.done() {
            let at = input.at;
            let id = input.u8()?;
            let mut sub = Input::new(input.sized()?);
            match id {
                SYMBOL_TABLE => self.symbols = sub.vector(|sub| self.symbol(sub))?,
                SEGMENT_INFO => {
                    let infos = sub.vector(|sub| {
                        let name = sub.name()?.to_owned();
                        let log = sub.uleb()?;
                        let flags = sub.uleb()?;
                        if flags & SEGMENT_TLS != 0 {
                            return Err(Malformed::Unsupported { what: Coded::Segment, code: flags });
                        }
                        Ok((name, 1u64.checked_shl(log).ok_or(Malformed::Leb { at: sub.at })?))
                    })?;
                    for (segment, (name, align)) in self.data.segments.iter_mut().zip(infos) {
                        segment.name = name;
                        segment.align = align;
                    }
                }
                INIT_FUNCS => {
                    self.inits = sub.vector(|sub| Ok(Init { priority: sub.uleb()?, symbol: sub.uleb()? }))?;
                }
                COMDAT_INFO => self.comdats = sub.vector(Input::comdat)?,
                code => return Err(Malformed::Code { what: Coded::Subsection, code: u32::from(code), at }),
            }
        }
        Ok(())
    }

    fn symbol(&self, input: &mut Input<'_>) -> Result<Symbol, Malformed> {
        let kind = input.coded(Coded::Symbol, SymbolKind::of)?;
        let flags = SymbolFlags(input.uleb()?);
        let undefined = flags.has(SymbolFlags::UNDEFINED);
        let (what, name) = match kind {
            SymbolKind::Data => {
                let name = input.name()?.to_owned();
                let def = if undefined {
                    None
                } else {
                    let segment = input.uleb()?;
                    Some(DataDef { segment, offset: input.uleb()?, size: input.uleb()? })
                };
                (Symbolic::Data(def), Some(name))
            }
            SymbolKind::Function | SymbolKind::Global | SymbolKind::Table => {
                let index = input.uleb()?;
                let named = !undefined || flags.has(SymbolFlags::EXPLICIT_NAME);
                let name = if named { Some(input.name()?.to_owned()) } else { self.imported(kind, index) };
                let what = match kind {
                    SymbolKind::Function => Symbolic::Function(index),
                    SymbolKind::Global => Symbolic::Global(index),
                    _ => Symbolic::Table(index),
                };
                (what, name)
            }
            SymbolKind::Section | SymbolKind::Tag => {
                input.uleb()?;
                if kind == SymbolKind::Tag && (!undefined || flags.has(SymbolFlags::EXPLICIT_NAME)) {
                    input.name()?;
                }
                (Symbolic::Other(kind as u8), None)
            }
        };
        Ok(Symbol { flags, name, what })
    }

    /// The name of the import at `index` among those of `kind`.
    fn imported(&self, kind: SymbolKind, index: u32) -> Option<String> {
        let of_kind = self.imports.iter().filter(|import| {
            matches!(
                (kind, import.import),
                (SymbolKind::Function, Import::Func(_))
                    | (SymbolKind::Global, Import::Global { .. })
                    | (SymbolKind::Table, Import::Table(_))
            )
        });
        of_kind.clone().nth(index as usize).map(|import| import.name.clone())
    }

    /// `relocs`, each of a field inside a payload of `len` bytes and of an index in range, by
    /// offset.
    fn checked(&self, mut relocs: Vec<Reloc>, len: usize) -> Result<Vec<Reloc>, Malformed> {
        relocs.sort_by_key(|r| r.offset);
        for reloc in &relocs {
            let width = match reloc.kind {
                RelocKind::TableIndexI32 | RelocKind::MemoryAddrI32 => I32,
                _ => LEB,
            };
            if reloc.offset + width > len {
                return Err(Malformed::Truncated { at: reloc.offset + width, end: len });
            }
            let (what, count) = match reloc.kind {
                RelocKind::TypeIndexLeb => (Coded::Type, self.types.len()),
                _ => (Coded::Symbol, self.symbols.len()),
            };
            if reloc.index as usize >= count {
                return Err(Malformed::Index { what, index: reloc.index });
            }
        }
        Ok(relocs)
    }
}
