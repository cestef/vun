//! Binding an object's relocations to its own definitions: a shared library reaches what it
//! defines as it is there, never through another module's.

use vun_index::Idx;

use crate::{Binding, Object, SectionId, Symbol, SymbolId, SymbolKind};

impl Object {
    /// Makes every relocation naming a symbol this object defines name its section instead, at
    /// the address the symbol is reached by, but those through the global offset table: the object's own definitions are reached as they are
    /// here, never through another module's, which a shared library needs of every symbol it
    /// exports. Storage per thread is reached by its offset in the thread's block, which linkers
    /// know of symbols of storage per thread alone, not of sections: a relocation naming one names
    /// a local symbol of it instead.
    ///
    /// # Panics
    /// Panics on a symbol at an offset past 2^63.
    pub fn bind_locally(&mut self) {
        self.bind_per_thread();
        // A section's symbol would take the offset as an addend, which the field must hold.
        if self.machine.implicit_addends() {
            return;
        }
        let targets: Vec<Option<(SectionId, u64)>> = self
            .symbols
            .iter()
            .map(|(_, s)| {
                let reached = self.machine.reached(s.kind, s.value);
                let through = !matches!(s.kind, SymbolKind::Section | SymbolKind::Tls);
                s.section.filter(|_| through).map(|at| (at, reached))
            })
            .collect();
        let mut sections = self.sections.iter().map(|(id, _)| id).collect::<Vec<_>>();
        sections.retain(|id| targets.iter().flatten().any(|(at, _)| at == id));
        let symbols: Vec<(SectionId, SymbolId)> =
            sections.into_iter().map(|s| (s, self.section_symbol(s))).collect();
        for (_, section) in self.sections.iter_mut() {
            for r in section.relocs.iter_mut().filter(|r| !r.kind.through_table()) {
                let Some((at, value)) = targets.get(r.symbol.index()).copied().flatten() else { continue };
                let (_, symbol) =
                    symbols.iter().find(|(s, _)| *s == at).expect("each defining section has one");
                r.symbol = *symbol;
                r.addend += i64::try_from(value).expect("offsets fit 63 bits");
            }
        }
    }

    /// Makes every relocation naming a global symbol of storage per thread this object defines
    /// name a local one at its place.
    fn bind_per_thread(&mut self) {
        let global =
            |s: &Symbol| s.kind == SymbolKind::Tls && s.section.is_some() && s.binding == Binding::Global;
        let exported: Vec<SymbolId> =
            self.symbols.iter().filter(|(_, s)| global(s)).map(|(id, _)| id).collect();
        for id in exported {
            let local = Symbol { binding: Binding::Local, ..self.symbols[id].clone() };
            let local = self.symbols.push(local);
            for (_, section) in self.sections.iter_mut() {
                for r in section.relocs.iter_mut().filter(|r| r.symbol == id) {
                    r.symbol = local;
                }
            }
        }
    }
}
