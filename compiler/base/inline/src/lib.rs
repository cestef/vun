//! Inline assembly blocks (spec 07, section 6): the instructions of one architecture as typed
//! values, checked against its table in `lang.arch.<arch>` when compiling, over operands that
//! bind values of whichever representation carries the block. Sema binds expressions, MIR binds
//! operands and places, and LIR values and the addresses of places; `V` is what an input binds,
//! `P` what an output binds, and `S` a symbol whose address an operand names.
//!
//! Instructions, forms, widths, conditions and register classes are named by their index in the
//! architecture's table, in declaration order, which the generated assembler tables keep.

use vun_codec::{Decode, Encode};
use vun_items::Item;
use vun_origin::OriginId;

mod rebind;
#[cfg(test)]
mod tests;
mod walk;

pub use rebind::Rebind;
pub use walk::Read;

/// An index in one of an architecture's tables, in declaration order.
macro_rules! index {
    ($($(#[$attr:meta])* $name:ident($int:ty);)*) => {$(
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
        pub struct $name(pub $int);
    )*};
}

index! {
    /// An instruction of `lang.arch.<arch>`.
    OpId(u16);
    /// One of an instruction's forms.
    FormId(u16);
    /// A width, of `lang.arch.<arch>.Size`.
    SizeId(u8);
    /// A condition, of `lang.arch.<arch>.Cond`, in encoding order.
    CondId(u8);
    /// A class of registers.
    ClassId(u8);
    /// A line of the block, which a label names.
    Line(u16);
}

/// A block of instructions: an expression of type `()`, or `never` when it does not return.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Block<V, P, S> {
    /// The namespace of the architecture, `lang.arch.<arch>`.
    pub arch: Item,
    pub insts: Vec<Inst<V, P, S>>,
    pub returns: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Inst<V, P, S> {
    pub op: OpId,
    /// The form the operands were checked against.
    pub form: FormId,
    pub size: SizeId,
    pub operands: Vec<Operand<V, P, S>>,
    pub origin: OriginId,
}

/// A register, by its class and its number in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct RegRef {
    pub class: ClassId,
    pub number: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Operand<V, P, S> {
    Reg(RegRef),
    Imm(i64),
    Cond(CondId),
    /// A named operand of another kind, by its index in the enum the form's position names, in
    /// declaration order: a barrier (`dmb ish`), an interrupt mask (`cpsid i`).
    Choice(u8),
    Mem(Mem<V, P, S>),
    Label(Line),
    /// The address of a declaration, relocated when linking.
    Symbol(S),
    Bound(Bound<V, P>),
}

/// Memory at `base + index * scale + disp`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Mem<V, P, S> {
    pub base: Option<Base<V, P>>,
    pub index: Option<(Base<V, P>, u8)>,
    pub disp: Disp<S>,
}

/// A register of an address: named, or bound to a value.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Base<V, P> {
    Reg(RegRef),
    Bound(Bound<V, P>),
}

/// The displacement of an address: bytes, or those of a symbol's address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Disp<S> {
    Bytes(i64),
    Symbol(S, i64),
}

/// A Vun value in a register: read by the block (`{in e}`), written into a place (`{out p}`), or
/// both (`{inout p}`), in the register named or one the backend picks.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Bound<V, P> {
    In { value: V, reg: Option<RegRef> },
    Out { place: P, reg: Option<RegRef> },
    InOut { place: P, reg: Option<RegRef> },
}

impl<V, P> Bound<V, P> {
    /// The register named for it, if one is.
    #[must_use]
    pub fn reg(&self) -> Option<RegRef> {
        match self {
            Self::In { reg, .. } | Self::Out { reg, .. } | Self::InOut { reg, .. } => *reg,
        }
    }
}
