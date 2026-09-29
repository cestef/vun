//! Low-level IR: type-erased SSA shared by every backend (spec 13, section 3). Values are
//! scalars; aggregates live in memory, reached by address. Functions, data and imports are
//! addressed by their ids, and only exports and imports carry link symbols.

pub mod analysis;
pub mod build;
mod compact;
pub mod entity;
pub mod facts;
pub mod fold;
pub mod func;
pub mod inst;
pub mod text;
pub mod trace;
pub mod ty;
pub mod unit;
mod uses;
pub mod verify;

pub use build::Builder;
pub use entity::{Block, DataId, FuncId, ImportId, Inst, Slot, Value};
pub use facts::{Domain, Reads};
pub use func::{AbiType, Alias, Conv, Ext, Function, Linkage, Record, Signature};
pub use inst::{
    Access, Address, Asm, AsmCheck, BinOp, Callee, CastOp, Cond, InstKind, Order, Output, Right, RmwOp,
    Target, Terminator,
};
pub use trace::{Field, Fields, Trace, TraceLayout};
pub use ty::Scalar;
pub use unit::{Data, Import, ImportKind, Reloc, Site, Storage, Unit};
pub use verify::Malformed;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
#[cfg(test)]
mod tests;
