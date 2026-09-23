//! Compilation units: functions, data, symbols.

use overt_origin::OriginId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linkage {
    Internal,
    /// Exported under its name (`#[link.export]`).
    Export,
    /// Provided from outside (`= _` with a link attribute).
    Import,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub linkage: Linkage,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub name: String,
    pub linkage: Linkage,
    /// Link section, if not the default one.
    pub section: Option<String>,
    /// Contents; `None` for zero-initialized data of `size` bytes.
    pub bytes: Option<Vec<u8>>,
    pub size: u64,
    pub align: u32,
    pub origin: OriginId,
}

/// What a backend receives: everything needed to emit one object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    pub functions: Vec<Function>,
    pub data: Vec<Data>,
}
