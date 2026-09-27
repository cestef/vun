//! C identifiers: link symbols as written, and the backend's own names for what it declares,
//! each spelled once, here. An own name that a link symbol or a keyword already spells gets
//! underscores appended until it is free; own spellings never end with one, so they stay
//! distinct from each other.

use std::borrow::Cow;
use vun_hash::Set;

use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Block, DataId, FuncId, Order, Scalar, Slot, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ident {
    /// A link symbol, as written.
    Symbol(Symbol),
    Own(Own),
    /// What the target's C compiler provides beyond C99, when its profile says it does.
    Builtin(Builtin),
}

/// The builtins of GCC and Clang the backend uses for atomic access (`lang.target.Atomics.gnu`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Builtin {
    Load,
    Store,
    Exchange,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
    CompareExchange,
    Order(Order),
}

impl Builtin {
    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Load => "__atomic_load_n",
            Self::Store => "__atomic_store_n",
            Self::Exchange => "__atomic_exchange_n",
            Self::FetchAdd => "__atomic_fetch_add",
            Self::FetchSub => "__atomic_fetch_sub",
            Self::FetchAnd => "__atomic_fetch_and",
            Self::FetchOr => "__atomic_fetch_or",
            Self::FetchXor => "__atomic_fetch_xor",
            Self::CompareExchange => "__atomic_compare_exchange_n",
            Self::Order(Order::Relaxed) => "__ATOMIC_RELAXED",
            Self::Order(Order::Acquire) => "__ATOMIC_ACQUIRE",
            Self::Order(Order::Release) => "__ATOMIC_RELEASE",
            Self::Order(Order::AcqRel) => "__ATOMIC_ACQ_REL",
            Self::Order(Order::SeqCst) => "__ATOMIC_SEQ_CST",
        }
    }
}

/// What the backend declares under its own names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Own {
    Func(u32),
    Data(u32),
    /// The `struct` laying out a data object's bytes and addresses.
    Parts(u32),
    Value(u32),
    Slot(u32),
    /// The array a slot aligned past every C type lies in.
    Backing(u32),
    Block(u32),
    Param(u32),
    /// A record passed by value under the C convention.
    Record(u32),
    Field(u32),
    /// A `struct` of this many bytes, copied as one.
    Copy(u64),
    /// A check compiled as a type.
    Check(u32),
    /// A function's record result under the C convention.
    Result,
    /// The temporary breaking cycles of moves of a type.
    Temp(Scalar),
    /// `uN` or `iN`: the integer of `bits` bits, declared from the target's C types.
    Int {
        bits: u32,
        signed: bool,
    },
    /// The `struct` whose size checks the alignment of `uN`, or of an address for 0.
    Aligned(u32),
    Helper(Helper),
}

/// A float conversion the file defines once it is used: reading a value's bits as the other type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Helper {
    /// The float of `bits` bits whose bits these are.
    FloatOf(u32),
    /// The bits of a float of `bits` bits.
    BitsOf(u32),
}

impl Ident {
    #[must_use]
    pub fn func(f: FuncId) -> Self {
        Self::Own(Own::Func(Own::number(f.index())))
    }

    #[must_use]
    pub fn data(d: DataId) -> Self {
        Self::Own(Own::Data(Own::number(d.index())))
    }

    #[must_use]
    pub fn parts(d: DataId) -> Self {
        Self::Own(Own::Parts(Own::number(d.index())))
    }

    #[must_use]
    pub fn value(v: Value) -> Self {
        Self::Own(Own::Value(Own::number(v.index())))
    }

    #[must_use]
    pub fn slot(s: Slot) -> Self {
        Self::Own(Own::Slot(Own::number(s.index())))
    }

    /// The array the slot `s` lies in, when it is aligned past every C type.
    #[must_use]
    pub fn backing(s: Slot) -> Self {
        Self::Own(Own::Backing(Own::number(s.index())))
    }

    #[must_use]
    pub fn block(b: Block) -> Self {
        Self::Own(Own::Block(Own::number(b.index())))
    }

    #[must_use]
    pub fn param(i: usize) -> Self {
        Self::Own(Own::Param(Own::number(i)))
    }

    #[must_use]
    pub fn record(i: usize) -> Self {
        Self::Own(Own::Record(Own::number(i)))
    }

    #[must_use]
    pub fn field(i: usize) -> Self {
        Self::Own(Own::Field(Own::number(i)))
    }
}

impl Own {
    /// Whether the linker sees it when a program spans several files: functions and data.
    const fn linked(self) -> bool {
        matches!(self, Self::Func(_) | Self::Data(_))
    }

    /// An index as the number in a name.
    fn number(i: usize) -> u32 {
        u32::try_from(i).expect("indices fit u32")
    }

    /// How it is written when nothing else is.
    fn spelling(self) -> String {
        match self {
            Self::Func(n) => format!("f{n}"),
            Self::Data(n) => format!("d{n}"),
            Self::Parts(n) => format!("dp{n}"),
            Self::Value(n) => format!("v{n}"),
            Self::Slot(n) => format!("s{n}"),
            Self::Backing(n) => format!("sb{n}"),
            Self::Block(n) => format!("b{n}"),
            Self::Param(n) => format!("p{n}"),
            Self::Record(n) => format!("r{n}"),
            Self::Field(n) => format!("m{n}"),
            Self::Copy(n) => format!("bytes{n}"),
            Self::Check(n) => format!("check{n}"),
            Self::Result => "result".to_owned(),
            Self::Temp(s) => format!("temp_{s}"),
            Self::Int { bits, signed } => format!("{}{bits}", if signed { "i" } else { "u" }),
            Self::Aligned(0) => "aligned_ptr".to_owned(),
            Self::Aligned(bits) => format!("aligned_u{bits}"),
            Self::Helper(Helper::FloatOf(bits)) => format!("float_of{bits}"),
            Self::Helper(Helper::BitsOf(bits)) => format!("bits_of{bits}"),
        }
    }
}

/// C99's keywords, which no name may spell.
const KEYWORDS: [&str; 37] = [
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Bool",
    "_Complex",
    "_Imaginary",
];

/// The spellings of the names of one program.
#[derive(Clone)]
pub struct Names {
    taken: Set<&'static str>,
    /// Marks the functions and data of a program spanning several files, which the linker then
    /// sees: `p1f3a_f3`, the program's own, never another's.
    tag: Option<u32>,
}

impl Names {
    /// The names of a program declaring or using `symbols`, tagged by `tag` when it spans
    /// several files.
    #[must_use]
    pub fn new(symbols: impl IntoIterator<Item = Symbol>, tag: Option<u32>) -> Self {
        Self { taken: KEYWORDS.into_iter().chain(symbols.into_iter().map(Symbol::as_str)).collect(), tag }
    }

    /// Whether a link symbol can be written in C as it is: an identifier, not a keyword.
    #[must_use]
    pub fn writable(symbol: Symbol) -> bool {
        let s = symbol.as_str();
        s.bytes().next().is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !KEYWORDS.contains(&s)
    }

    #[must_use]
    pub fn spell(&self, ident: Ident) -> Cow<'static, str> {
        match ident {
            Ident::Symbol(s) => Cow::Borrowed(s.as_str()),
            Ident::Builtin(b) => Cow::Borrowed(b.spelling()),
            Ident::Own(own) => {
                let mut name = match self.tag {
                    Some(tag) if own.linked() => format!("p{tag:x}_{}", own.spelling()),
                    _ => own.spelling(),
                };
                while self.taken.contains(name.as_str()) {
                    name.push('_');
                }
                Cow::Owned(name)
            }
        }
    }
}
