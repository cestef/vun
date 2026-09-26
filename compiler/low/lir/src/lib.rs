//! Low-level IR: type-erased SSA shared by every backend (spec 13, section 3). Values are
//! scalars; aggregates live in memory, reached by address. Functions, data and imports are
//! addressed by their ids, and only exports and imports carry link symbols.

pub mod analysis;
pub mod build;
mod compact;
pub mod entity;
pub mod facts;
mod fold;
pub mod func;
pub mod inline;
pub mod inst;
pub mod legalize;
mod narrow;
pub mod promote;
mod replaced;
mod reuse;
pub mod scalarize;
mod simplify;
mod sweep;
pub mod text;
pub mod thread;
pub mod ty;
pub mod unit;
mod uses;
pub mod verify;

pub use build::Builder;
pub use entity::{Block, DataId, FuncId, ImportId, Inst, Slot, Value};
pub use facts::{Domain, Reads};
pub use func::{AbiType, Alias, Conv, Ext, Function, Linkage, Record, Signature};
pub use inst::{
    Access, Address, BinOp, Callee, CastOp, Cond, InstKind, Order, Right, RmwOp, Target, Terminator,
};
pub use ty::Scalar;
pub use unit::{Data, Import, ImportKind, Reloc, Site, Unit};
pub use verify::Malformed;

#[cfg(test)]
mod tests;
