//! How code names what it calls and addresses: a Cranelift user name per kind of address, which
//! the relocations of the compiled code give back.

use cranelift_codegen::ir::UserExternalName;
use vun_index::Idx;
use vun_lir::{Address, DataId, FuncId, ImportId};

/// The namespace of each kind of address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Space {
    Func,
    Data,
    Import,
    /// The tail-convention body of a function that has an entry (`Tails`).
    Body,
}

impl Space {
    const ALL: [Self; 4] = [Self::Func, Self::Data, Self::Import, Self::Body];

    const fn number(self) -> u32 {
        self as u32
    }
}

/// The body of a function whose entry is an address: its code in the tail convention, where the
/// entry's code ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Body(pub(crate) FuncId);

/// What a name stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Address(Address),
    Body(FuncId),
}

/// The name of an address in Cranelift's functions.
pub(crate) trait Named {
    fn name(self) -> UserExternalName;
}

impl Named for Address {
    fn name(self) -> UserExternalName {
        let (space, index) = match self {
            Self::Func(f) => (Space::Func, f.index()),
            Self::Data(d) => (Space::Data, d.index()),
            Self::Import(i) => (Space::Import, i.index()),
        };
        UserExternalName::new(space.number(), u32::try_from(index).expect("ids fit u32"))
    }
}

impl Named for Body {
    fn name(self) -> UserExternalName {
        UserExternalName::new(Space::Body.number(), u32::try_from(self.0.index()).expect("ids fit u32"))
    }
}

/// What a name stands for.
pub(crate) trait Targeted {
    fn target(&self) -> Target;
}

impl Targeted for UserExternalName {
    fn target(&self) -> Target {
        let index = self.index as usize;
        match Space::ALL.into_iter().find(|s| s.number() == self.namespace) {
            Some(Space::Func) => Target::Address(Address::Func(FuncId::new(index))),
            Some(Space::Data) => Target::Address(Address::Data(DataId::new(index))),
            Some(Space::Import) => Target::Address(Address::Import(ImportId::new(index))),
            Some(Space::Body) => Target::Body(FuncId::new(index)),
            None => panic!("the backend names only the addresses of a unit"),
        }
    }
}
