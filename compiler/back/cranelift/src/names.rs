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
}

impl Space {
    const ALL: [Self; 3] = [Self::Func, Self::Data, Self::Import];

    const fn number(self) -> u32 {
        self as u32
    }
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

/// The address a name stands for.
pub(crate) trait Addressed {
    fn address(&self) -> Address;
}

impl Addressed for UserExternalName {
    fn address(&self) -> Address {
        let index = self.index as usize;
        match Space::ALL.into_iter().find(|s| s.number() == self.namespace) {
            Some(Space::Func) => Address::Func(FuncId::new(index)),
            Some(Space::Data) => Address::Data(DataId::new(index)),
            Some(Space::Import) => Address::Import(ImportId::new(index)),
            None => panic!("the backend names only the addresses of a unit"),
        }
    }
}
