//! Symbols: the entries of a symbol table after its null one, locals first, each named in the
//! string table the symbol table links.

use vun_bytes::{Out, Strings};

use crate::headers::SymbolEntry;
use crate::input::{Input, Malformed};
use crate::record::Word;
use crate::{Class, File, SectionKind};

numbered! {
    /// What a symbol names (`STT_*`).
    pub enum SymbolKind: u8 {
        NoType = 0,
        Object = 1,
        Func = 2,
        Section = 3,
        File = 4,
        /// Storage per thread, by its offset in the thread's block.
        Tls = 6,
    }
}

numbered! {
    /// Where a symbol is seen (`STB_*`).
    pub enum Binding: u8 {
        Local = 0,
        Global = 1,
        Weak = 2,
    }
}

/// A symbol: its name, kind and binding, the number of the section defining it (0 where none
/// does), its value there and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: Vec<u8>,
    pub kind: SymbolKind,
    pub binding: Binding,
    pub section: u16,
    pub value: u64,
    pub size: u64,
}

/// The low bits of a symbol's info, its kind; the high bits are its binding.
const KIND_BITS: u8 = 4;

impl Symbol {
    /// The size of a symbol table's entry in `class`.
    #[must_use]
    pub fn size(class: Class) -> u64 {
        SymbolEntry::size(class)
    }

    /// The bytes of a symbol table of `class` holding `symbols`, and of its string table.
    #[must_use]
    pub fn table(class: Class, symbols: &[Self]) -> (Vec<u8>, Vec<u8>) {
        let (mut table, mut strings) = (Out::default(), Strings::default());
        SymbolEntry::default().put(class, &mut table);
        for symbol in symbols {
            let entry = SymbolEntry {
                name: strings.add(&symbol.name),
                info: symbol.binding.number() << KIND_BITS | symbol.kind.number(),
                other: 0,
                section: symbol.section,
                value: Word(symbol.value),
                size: Word(symbol.size),
            };
            entry.put(class, &mut table);
        }
        (table.0, strings.into_bytes())
    }

    /// The symbol of `entry`, named in `strings`.
    fn of(entry: SymbolEntry, strings: &[u8]) -> Result<Self, Malformed> {
        Ok(Self {
            name: Input::string(strings, u64::from(entry.name))?.to_vec(),
            kind: SymbolKind::of(entry.info & ((1 << KIND_BITS) - 1)),
            binding: Binding::of(entry.info >> KIND_BITS),
            section: entry.section,
            value: entry.value.0,
            size: entry.size.0,
        })
    }
}

impl File {
    /// The symbols of its symbol table, if it has one.
    ///
    /// # Errors
    /// Where the table links no string table, or holds a name past it.
    pub fn symbols(&self) -> Result<Vec<Symbol>, Malformed> {
        let Some(table) = self.sections.iter().find(|s| s.kind == SectionKind::SymTab) else {
            return Ok(Vec::new());
        };
        let strings = self.numbered(table.link).ok_or(Malformed::Link(table.link))?.bytes();
        let size = SymbolEntry::size(self.class);
        let mut input = Input::at(table.bytes(), size);
        (1..table.bytes().len() as u64 / size)
            .map(|_| Symbol::of(SymbolEntry::take(self.class, &mut input)?, strings))
            .collect()
    }

    /// The symbol named `name`, if the file has one.
    ///
    /// # Errors
    /// As [`File::symbols`].
    pub fn symbol(&self, name: &[u8]) -> Result<Option<Symbol>, Malformed> {
        Ok(self.symbols()?.into_iter().find(|s| s.name == name))
    }
}
