//! The table of the program's code (`lang.trace`, spec 07, section 3). Lowering makes the datum a
//! runtime reads it at and records how `lang.trace` lays out its structs; the table is written
//! when code is generated, where each origin's place in source is known.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

use crate::entity::DataId;

/// The table of a program whose runtime reads it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Trace {
    /// The datum of `lang.trace.table`, whose bytes are written with the rows.
    pub table: DataId,
    pub layout: TraceLayout,
    /// What the table describes: the functions only at `Detail::Calls`.
    pub detail: vun_target::Detail,
}

/// How `lang.trace` lays out each of its structs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct TraceLayout {
    pub table: Fields,
    pub function: Fields,
    pub site: Fields,
    pub origin: Fields,
    pub text: Fields,
}

/// A struct's size and alignment, and where each of its fields lies.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Fields {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<Field>,
}

/// A field: its name, and the offset and size of its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Field {
    pub name: Symbol,
    pub offset: u64,
    pub bytes: u64,
}

impl Fields {
    /// The field named `name`.
    #[must_use]
    pub fn field(&self, name: Symbol) -> Option<Field> {
        self.fields.iter().copied().find(|f| f.name == name)
    }
}
