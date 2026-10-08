//! Lowering a program's MIR to LIR (spec 13, section 3): the functions its exports reach, each
//! from its concrete MIR, with range integers in their storage, aggregates and enums in memory
//! by their shapes, and trap sites calling `lang.panic.trap`.

mod assemble;
mod frame;
mod func;
mod inlined;
mod lowering;
mod order;
mod piece;
mod placing;
mod program;
mod repr;
mod signature;
mod unchecked;
mod unlowered;

pub use frame::{FrameShape, FrameState, Frames, Framing};
pub use inlined::{InlinedLir, Inlining};
pub use piece::{Bytes, Datum, DatumKind, FuncRef, FunctionLir, Lir, Piece, Refs, SiteAt};
pub use program::{EachLowered, Lowered};
pub use repr::{Repr, Types};
pub use unchecked::Unchecked;
pub use unlowered::{Form, Unlowered, Why};

/// The queries of lowering whose memos outlive the process, with their values.
pub const PERSISTED: &[fn(&vun_query::Db)] = &[
    vun_query::Db::persist::<FunctionLir>,
    vun_query::Db::persist::<InlinedLir>,
    vun_query::Db::persist::<Lowered>,
    vun_query::Db::persist::<EachLowered>,
    vun_query::Db::persist::<Frames>,
];
