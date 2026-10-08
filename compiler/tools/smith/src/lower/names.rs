//! What the program's names are called.

use crate::ir::{FnId, Label, LocalId, TypeId};
use crate::ty::Nominal;
use vun_index::Idx;

/// What a name stands for.
#[derive(Clone, Copy)]
enum Role {
    Function,
    Param,
    Local,
    Root,
    Struct,
    Enum,
    Field,
    Binder,
    Generic,
    Label,
}

impl Role {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Function => "f",
            Self::Param => "a",
            Self::Local => "v",
            Self::Root => "r",
            Self::Struct => "S",
            Self::Enum => "E",
            Self::Field => "x",
            Self::Binder => "p",
            Self::Generic => "T",
            Self::Label => "l",
        }
    }

    fn name(self, index: usize) -> String {
        format!("{}{index}", self.prefix())
    }
}

pub(super) fn function(id: FnId) -> String {
    Role::Function.name(id.index())
}

/// The name `main` gives the value the `n`th root made.
pub(super) fn root(n: usize) -> String {
    Role::Root.name(n)
}

/// The name of a local of a function with `params` parameters.
pub(super) fn local(id: LocalId, params: usize) -> String {
    let role = if id.index() < params { Role::Param } else { Role::Local };
    role.name(id.index())
}

/// The name of a struct or an enum of the program.
pub(super) fn ty(id: TypeId, structure: bool) -> String {
    (if structure { Role::Struct } else { Role::Enum }).name(id.index())
}

/// The name of the `at`th type parameter of a function.
pub(super) fn generic(at: usize) -> String {
    Role::Generic.name(at)
}

/// The name of a loop, without the quote that writes it as a label.
pub(super) fn label(label: Label) -> String {
    Role::Label.name(label.0)
}

pub(super) fn field(at: usize) -> String {
    Role::Field.name(at)
}

pub(super) fn variant(nominal: Nominal, at: usize) -> String {
    nominal.variant(at)
}

/// The name `main` gives the `at`th part of a payload, `depth` enums down.
pub(super) fn binder(depth: usize, at: usize) -> String {
    format!("{}{depth}_{at}", Role::Binder.prefix())
}
