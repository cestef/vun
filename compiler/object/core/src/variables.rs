//! Mach-O's thread-local variables: storage per thread is reached through a descriptor of each, in
//! a section of their own, whose function the loader replaces with the one giving the calling
//! thread's storage. A variable's symbol names its descriptor, and the storage's initial bytes
//! have a symbol of their own.

use crate::{Binding, Contents, Object, Reloc, Symbol, SymbolId, SymbolKind};

/// The section of the descriptors.
const VARIABLES: &[u8] = b"__thread_vars";
/// The function each descriptor names until the loader replaces it (`_tlv_bootstrap`, Mach-O's
/// underscore aside).
const BOOTSTRAP: &[u8] = b"_tlv_bootstrap";
/// What a variable's initial bytes are named, after the variable.
const INITIAL: &[u8] = b"$tlv$init";

/// A descriptor's words: the function, a key the loader writes, and the initial bytes' address,
/// by their indices.
const WORDS: u8 = 3;
const FUNCTION: u64 = 0;
const BYTES: u64 = 2;

impl Object {
    /// Gives each symbol of storage per thread this object defines a descriptor, which the
    /// symbol then names, its initial bytes named by a local symbol of their own.
    pub fn thread_variables(&mut self) {
        let defined: Vec<SymbolId> = self
            .symbols
            .iter()
            .filter(|(_, s)| s.kind == SymbolKind::Tls && s.section.is_some())
            .map(|(id, _)| id)
            .collect();
        if defined.is_empty() {
            return;
        }
        let word = self.machine.address_bytes();
        let bytes = u64::from(word);
        let variables = self.section_named(VARIABLES, Contents::ThreadVariables);
        let bootstrap = self.symbols.push(Symbol {
            name: BOOTSTRAP.to_vec(),
            section: None,
            value: 0,
            size: 0,
            kind: SymbolKind::Func,
            binding: Binding::Global,
        });
        for id in defined {
            let variable = &self.symbols[id];
            let name = [variable.name.as_slice(), INITIAL].concat();
            let initial =
                Symbol { name, kind: SymbolKind::Object, binding: Binding::Local, ..variable.clone() };
            let initial = self.symbols.push(initial);
            let size = u64::from(WORDS) * bytes;
            let at = self.append(variables, &vec![0; usize::from(WORDS) * usize::from(word)], bytes);
            let kind = self.machine.address();
            for (word, symbol) in [(FUNCTION, bootstrap), (BYTES, initial)] {
                let reloc = Reloc { offset: at + word * bytes, symbol, kind, addend: 0 };
                self.sections[variables].relocs.push(reloc);
            }
            let variable = &mut self.symbols[id];
            (variable.section, variable.value, variable.size) = (Some(variables), at, size);
        }
    }
}
