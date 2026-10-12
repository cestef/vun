//! Compilation units: what a backend receives to emit one object. Functions, data and imports are
//! addressed by their ids; only exports and imports carry link symbols.

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::entity::{DataId, FuncId, ImportId};
use crate::func::{Function, Linkage, Signature};
use crate::inst::{Address, Callee, InstKind};

/// Initialized memory: bytes, with the addresses written into them at link time.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Data {
    pub linkage: Linkage,
    pub bytes: Vec<u8>,
    /// Addresses stored in the bytes, by the offset of the word holding each.
    pub relocs: Vec<Reloc>,
    pub align: u64,
    /// Whether the program writes it: a `mut` top-level binding (spec 05, section 4).
    pub writable: bool,
    /// Link section, when not the default one (`#[link.section]`).
    pub section: Option<Symbol>,
    pub storage: Storage,
    pub origin: OriginId,
}

/// Where a datum lives (spec 07, section 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Storage {
    /// Once, for the program.
    #[default]
    Program,
    /// Once for each thread, each starting as the datum's bytes.
    Thread,
}

/// The address of `target`, plus `addend` bytes, written at `offset` of a data object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Reloc {
    pub offset: u64,
    pub target: Address,
    pub addend: i64,
}

/// What a linked symbol is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum ImportKind {
    Func(Signature),
    Data,
}

/// A symbol the unit uses and something else defines (`name: T = _;` with `#[link.symbol]`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Import {
    pub symbol: Symbol,
    /// The library it comes from (`#[link.library(name)]`): a wasm import module, a library an
    /// ELF program links.
    pub library: Option<Symbol>,
    pub kind: ImportKind,
    pub origin: OriginId,
    /// The most bytes of stack a call of it takes, as its declaration states (`#[stack.max(n)]`).
    pub stack_max: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unit {
    pub functions: IndexVec<FuncId, Function>,
    pub data: IndexVec<DataId, Data>,
    pub imports: IndexVec<ImportId, Import>,
    /// The width of addresses on the target, in bits.
    pub pointer_bits: u32,
    /// Each trap site, by its id (`lang.panic.Trap.site`): the message table that reports a
    /// trap (spec 07, section 3).
    pub sites: Vec<Site>,
    /// The checks of unsafe code the program is built with (spec 13, section 8.7): a backend
    /// checks the memory its assembly blocks write under `asm`.
    pub checks: vun_target::Checks,
    /// The table of the program's code, where its runtime reads it (`lang.trace`).
    pub trace: Option<crate::trace::Trace>,
}

impl Unit {
    /// The signature of every function the unit defines, imports and calls through an address,
    /// with where it is written.
    pub fn signatures(&self) -> impl Iterator<Item = (OriginId, &Signature)> + '_ {
        let defined = self.functions.iter().map(|(_, f)| (f.origin, &f.signature));
        let imported = self.imports.iter().filter_map(|(_, i)| match &i.kind {
            ImportKind::Func(signature) => Some((i.origin, signature)),
            ImportKind::Data => None,
        });
        let called = self.functions.iter().flat_map(|(_, f)| f.insts.iter()).filter_map(|(_, inst)| {
            match &inst.kind {
                InstKind::Call { callee: Callee::Indirect(_, signature), .. } => {
                    Some((inst.origin, &**signature))
                }
                _ => None,
            }
        });
        defined.chain(imported).chain(called)
    }

    /// The libraries its imports come from, each once, in the order they are first used.
    #[must_use]
    pub fn libraries(&self) -> Vec<Symbol> {
        let mut libraries: Vec<Symbol> = Vec::new();
        for library in self.imports.iter().filter_map(|(_, import)| import.library) {
            if !libraries.contains(&library) {
                libraries.push(library);
            }
        }
        libraries
    }

    /// The link symbols the unit exports: its functions' and its data's, in that order.
    pub fn exports(&self) -> impl Iterator<Item = Symbol> + '_ {
        let functions = self.functions.iter().map(|(_, f)| f.linkage);
        let data = self.data.iter().map(|(_, d)| d.linkage);
        functions.chain(data).filter_map(|linkage| match linkage {
            Linkage::Export(symbol) => Some(symbol),
            Linkage::Internal => None,
        })
    }
}

/// A trap site: where it is written, and what it says when its reason is known when compiling
/// (`lang.panic.fail`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Site {
    pub at: OriginId,
    pub msg: Option<vun_diag::Msg>,
}
