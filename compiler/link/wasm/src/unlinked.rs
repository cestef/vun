//! Why objects do not link, each reason typed.

use std::fmt;

use vun_wasm::Malformed;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unlinked {
    /// An object the linker cannot read, by its place among the inputs.
    Malformed { input: usize, why: Malformed },
    /// A symbol no object defines and no library provides, or one defined as another kind than
    /// it is used as.
    Undefined(String),
    /// A symbol two objects define, neither weakly.
    Duplicate(String),
    /// A function imported from one library under one name with two types.
    Clash { module: String, name: String },
    /// A relocation against a symbol of a kind it cannot name.
    Relocation { input: usize, symbol: u32 },
}

impl fmt::Display for Unlinked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { input, why } => {
                write!(f, "input {input} is not an object the linker reads: {why}")
            }
            Self::Undefined(name) => write!(f, "`{name}` is defined nowhere"),
            Self::Duplicate(name) => write!(f, "`{name}` is defined twice"),
            Self::Clash { module, name } => write!(f, "`{module}.{name}` is imported with two types"),
            Self::Relocation { input, symbol } => {
                write!(f, "input {input} relocates against symbol {symbol}, of a kind no relocation names")
            }
        }
    }
}

impl std::error::Error for Unlinked {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed { why, .. } => Some(why),
            _ => None,
        }
    }
}
