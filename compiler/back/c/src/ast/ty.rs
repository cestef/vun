//! C types, as the backend writes them.

use super::Ident;

/// A C integer type by its keywords.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Base {
    Char,
    Short,
    Int,
    Long,
    LongLong,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Void,
    /// A C integer type by its keywords, `unsigned long`.
    Base {
        base: Base,
        signed: bool,
    },
    /// `uN` or `iN`, declared from the target's C types.
    Int {
        bits: u32,
        signed: bool,
    },
    /// `__int128`, which GCC and Clang declare on 64-bit targets and pass as their conventions
    /// pass an integer twice a register wide.
    Wide {
        signed: bool,
    },
    Float,
    Double,
    Pointer(Box<Ty>),
    Volatile(Box<Ty>),
    /// A pointer through which alone what it reaches is reached (C99's `restrict`).
    Restrict(Box<Ty>),
    Struct(Ident),
    Union(Ident),
    /// A pointer to a function taking `params` and giving `result`.
    Function {
        result: Box<Ty>,
        params: Vec<Ty>,
    },
}

impl Ty {
    pub const BYTE: Self = Self::Int { bits: 8, signed: false };

    #[must_use]
    pub fn uint(bits: u32) -> Self {
        Self::Int { bits, signed: false }
    }

    #[must_use]
    pub fn int(bits: u32) -> Self {
        Self::Int { bits, signed: true }
    }

    /// `unsigned`: operands narrower than `int` become it before arithmetic, so promotion never
    /// makes unsigned arithmetic signed.
    #[must_use]
    pub fn unsigned() -> Self {
        Self::Base { base: Base::Int, signed: false }
    }

    #[must_use]
    pub fn pointer(self) -> Self {
        Self::Pointer(Box::new(self))
    }

    /// `u8 *`: an address.
    #[must_use]
    pub fn address() -> Self {
        Self::BYTE.pointer()
    }

    /// Whether the type is a pointer, whose qualifiers C writes after its `*`.
    #[must_use]
    pub fn derived(&self) -> bool {
        match self {
            Self::Pointer(_) | Self::Function { .. } => true,
            Self::Volatile(ty) | Self::Restrict(ty) => ty.derived(),
            _ => false,
        }
    }

    /// `void (*)(void)`: a function's address held in data.
    #[must_use]
    pub fn code() -> Self {
        Self::Function { result: Box::new(Self::Void), params: Vec::new() }
    }
}

/// The suffix giving an unsigned integer constant the C type it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suffix {
    Unsigned,
    Long,
    LongLong,
}
