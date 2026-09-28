//! The symbol table: locals, then symbols defined here, then those defined elsewhere, as the
//! dynamic symbol table's ranges say, each a C name with Mach-O's underscore.

use vun_index::IndexVec;

use super::{DEFINED, EXTERNAL, Placed, Put};
use crate::write::Strings;
use crate::{Binding, Object, SectionId, SymbolId, SymbolKind};

/// The symbols as the table orders them, locals, then those defined here, then the others, and
/// each one's number.
pub(super) struct Order {
    pub(super) symbols: Vec<SymbolId>,
    pub(super) number: IndexVec<SymbolId, Option<u32>>,
    pub(super) locals: u32,
    pub(super) defined: u32,
}

impl Object {
    pub(super) fn macho_order(&self) -> Order {
        // Sections and mapping symbols are ELF's; Mach-O names neither.
        let named = |(_, s): &(SymbolId, &crate::Symbol)| {
            !matches!(s.kind, SymbolKind::Section | SymbolKind::Mapping(_))
        };
        let group = |local: bool, defined: bool| {
            self.symbols
                .iter()
                .filter(named)
                .filter(move |(_, s)| {
                    (s.binding == Binding::Local) == local && s.section.is_some() == defined
                })
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        };
        let (locals, defined, undefined) = (group(true, true), group(false, true), group(false, false));
        let count = |v: &Vec<SymbolId>| u32::try_from(v.len()).expect("symbols fit 32 bits");
        let (l, d) = (count(&locals), count(&defined));
        let symbols: Vec<SymbolId> = [locals, defined, undefined].concat();
        let mut number: IndexVec<SymbolId, Option<u32>> = IndexVec::filled(None, self.symbols.len());
        for (k, &id) in (0..).zip(&symbols) {
            number[id] = Some(k);
        }
        Order { symbols, number, locals: l, defined: d }
    }

    /// The symbol table's entries and its names.
    pub(super) fn macho_symbols(
        &self,
        order: &Order,
        placed: &[Placed],
        ordinal: impl Fn(SectionId) -> Option<usize>,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut strings = Strings::default();
        let mut table = Vec::new();
        for &id in &order.symbols {
            let s = &self.symbols[id];
            table.put32(strings.add(&[b"_".as_slice(), &s.name].concat()));
            let external = if s.binding == Binding::Global { EXTERNAL } else { 0 };
            let (kind, sect, value) = match s.section.and_then(|sec| Some((sec, ordinal(sec)?))) {
                Some((sec, k)) => {
                    let addr = placed.iter().find(|p| p.id == sec).expect("placed").addr;
                    (DEFINED | external, u8::try_from(k).expect("255 sections at most"), addr + s.value)
                }
                None => (external, 0, 0),
            };
            table.extend_from_slice(&[kind, sect, 0, 0]);
            table.put64(value);
        }
        let mut strings = strings.0;
        if strings.is_empty() {
            strings.push(0);
        }
        strings.resize(strings.len().next_multiple_of(8), 0);
        (table, strings)
    }
}
