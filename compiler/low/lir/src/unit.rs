//! Compilation units: what a backend receives to emit one object. Functions, data and imports are
//! addressed by their ids; only exports and imports carry link symbols.

use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_origin::OriginId;

use crate::entity::{DataId, FuncId, ImportId};
use crate::func::{Function, Linkage, Signature};
use crate::inst::Address;

/// Initialized memory: bytes, with the addresses written into them at link time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    pub origin: OriginId,
}

/// The address of `target`, plus `addend` bytes, written at `offset` of a data object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reloc {
    pub offset: u64,
    pub target: Address,
    pub addend: i64,
}

/// What a linked symbol is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImportKind {
    Func(Signature),
    Data,
}

/// A symbol the unit uses and something else defines (`name: T = _;` with `#[link.symbol]`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Import {
    pub symbol: Symbol,
    pub kind: ImportKind,
    pub origin: OriginId,
    /// The most bytes of stack a call of it takes, as its declaration states (`#[stack.max(n)]`).
    pub stack_max: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Unit {
    pub functions: IndexVec<FuncId, Function>,
    pub data: IndexVec<DataId, Data>,
    pub imports: IndexVec<ImportId, Import>,
    /// The width of addresses on the target, in bits.
    pub pointer_bits: u32,
    /// Each trap site, by its id (`lang.panic.Trap.site`): the message table that reports a
    /// trap (spec 07, section 3).
    pub sites: Vec<Site>,
}

/// A trap site: where it is written, and what it says when its reason is known when compiling
/// (`lang.panic.fail`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Site {
    pub at: OriginId,
    pub msg: Option<vun_diag::Msg>,
}
