//! Conventions as data: the rules of each, which this crate declares, and the registers they name,
//! which each backend declares for its machine.

/// A calling convention on one machine: its rules and the machine's registers it uses. `I` is a
/// general register, `F` a floating-point one.
#[derive(Clone, Copy, Debug)]
pub struct Convention<I: 'static, F: 'static> {
    pub rules: &'static Rules,
    pub registers: Registers<I, F>,
}

/// The registers a convention names, in the order it takes them.
#[derive(Clone, Copy, Debug)]
pub struct Registers<I: 'static, F: 'static> {
    pub int_args: &'static [I],
    pub float_args: &'static [F],
    pub int_results: &'static [I],
    pub float_results: &'static [F],
    /// The register of its own the address of a result in memory travels in (`x8` under
    /// AAPCS64); without one it takes the first general argument register, and the callee gives
    /// it back as its result (System V).
    pub result_address: Option<I>,
    /// The general registers a call preserves; a call may overwrite every other.
    pub preserved: &'static [I],
    /// The general registers the platform keeps for itself, which hold no value.
    pub reserved: &'static [I],
}

/// What a convention says, apart from which registers it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub records: Records,
    /// The largest record travelling in registers; a larger one is [`Rules::large`].
    pub in_registers: u64,
    pub large: Large,
    pub short: Short,
    pub pair: Pair,
    pub stack: Stack,
    pub variadic: Variadic,
}

/// How a record of up to [`Rules::in_registers`] bytes travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Records {
    /// In eightbytes, each in a floating-point register when every field in it is a float, else
    /// in a general one (System V, section 3.2.3).
    Eightbytes,
    /// Up to `max` floats of one type, a field per floating-point register (a homogeneous
    /// aggregate); any other record a word per general register (AAPCS64, section 6.8.2).
    Homogeneous { max: usize },
}

/// What becomes of a record larger than [`Rules::in_registers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Large {
    /// Copied onto the stack whole.
    Stack,
    /// Copied by the caller, its address passed as a word.
    Address,
}

/// What a value finding too few registers of its class left does to the rest of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Short {
    /// Nothing: later values take them (System V).
    Keeps,
    /// No later value takes one (AAPCS64, rules C.11 and C.13).
    Closes,
}

/// Where an integer of two words travels in registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pair {
    /// In the next two general registers.
    Any,
    /// In the next even and odd pair, skipping an odd register (AAPCS64, rule C.9).
    Even,
}

/// How a C call's scalars lie on the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    /// A word each.
    Words,
    /// At their own size and alignment (Apple's arm64 variant).
    Packed,
}

/// What a call to a variadic function states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variadic {
    Nothing,
    /// How many floating-point registers carry arguments (System V's `al`).
    FloatCount,
}

/// The width of an address and of a general register on the machines built.
pub(crate) const POINTER_BITS: u32 = 64;
/// The bytes of a stack slot and of a general register.
pub(crate) const WORD: u64 = POINTER_BITS as u64 / 8;
/// The most a value on the stack is aligned to.
pub(crate) const STACK_ALIGN: u64 = 16;

impl Rules {
    /// The System V AMD64 convention.
    pub const SYSV: Self = Self {
        records: Records::Eightbytes,
        in_registers: 2 * WORD,
        large: Large::Stack,
        short: Short::Keeps,
        pair: Pair::Any,
        stack: Stack::Words,
        variadic: Variadic::FloatCount,
    };

    /// The procedure call standard for arm64 (AAPCS64).
    pub const AAPCS64: Self = Self {
        records: Records::Homogeneous { max: 4 },
        in_registers: 2 * WORD,
        large: Large::Address,
        short: Short::Closes,
        pair: Pair::Even,
        stack: Stack::Words,
        variadic: Variadic::Nothing,
    };

    /// Apple's variant of AAPCS64, on macOS and iOS: a C call's scalars packed on the stack, and
    /// an integer of two words in any two general registers.
    pub const DARWIN_ARM64: Self = Self { stack: Stack::Packed, pair: Pair::Any, ..Self::AAPCS64 };
}
