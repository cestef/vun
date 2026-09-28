//! What a convention says, apart from which registers it names: one value of [`Rules`] per
//! convention built.

/// What a convention says, apart from which registers it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// The bytes of a general register, of an address and of a slot of the stack.
    pub word: u64,
    /// The alignment of the stack at a call, the most a value on it is aligned to.
    pub stack_align: u64,
    pub records: Records,
    /// The largest record passed in registers; a larger one is [`Rules::large`]. A homogeneous
    /// aggregate travels in floating-point registers whatever its size.
    pub in_registers: u64,
    /// The largest record returned in registers, a homogeneous aggregate aside; a larger one is
    /// written through an address the caller passes.
    pub returned: u64,
    pub large: Large,
    pub short: Short,
    pub pair: Pair,
    pub floats: Floats,
    pub split: Split,
    pub stack: Stack,
    pub variadic: Variadic,
}

/// How a record small enough to travel in registers does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Records {
    /// In eightbytes, each in a floating-point register when every field in it is a float, else
    /// in a general one (System V, section 3.2.3).
    Eightbytes,
    /// Up to `max` floats of one type, a field per floating-point register (a homogeneous
    /// aggregate); any other record a word per general register (AAPCS64, section 6.8.2; AAPCS,
    /// section 6.1.2.1).
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
    /// No later value takes one (AAPCS64, rules C.11 and C.13; AAPCS, rules C.2 and C.6).
    Closes,
}

/// Where a value aligned to two words starts in general registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pair {
    /// At the next one.
    Any,
    /// At the next even one, skipping an odd one (AAPCS64, rule C.9; AAPCS, rule C.3).
    Even,
}

/// Where floats travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floats {
    /// Each in the next floating-point register, whatever its width (System V, AAPCS64).
    Next,
    /// In general registers, as integers of their width: a soft-float convention (AAPCS's base
    /// standard).
    Core,
    /// In floating-point registers each as wide as the narrowest float, a wider float taking an
    /// aligned run of them named by its first; each value takes the lowest run free, filling
    /// back a register an earlier alignment skipped (AAPCS's VFP variant, rule C.1).
    Backfill,
}

/// Whether a record finding too few general registers left may take them and the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    Never,
    /// Its first words in the registers left and the rest on the stack, while no argument is on
    /// the stack yet (AAPCS, rule C.5).
    WhileStackEmpty,
}

/// How a C call's scalars lie on the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stack {
    /// A word each, or the words they take.
    Words,
    /// At their own size and alignment (Apple's arm64 variant).
    Packed,
}

/// How a call of a C function passes its variadic arguments (`...`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variadic {
    /// As fixed ones (AAPCS64).
    Fixed,
    /// As fixed ones, the call stating how many floating-point registers carry arguments
    /// (System V's `al`).
    FloatCount,
    /// Each on the stack in the words it takes, whatever registers are left (Apple's arm64
    /// variant).
    Stack,
    /// As the base standard passes them, fixed arguments and result too: floats in general
    /// registers (AAPCS, section 6.5).
    Core,
}

impl Rules {
    /// The System V AMD64 convention.
    pub const SYSV: Self = Self {
        word: 8,
        stack_align: 16,
        records: Records::Eightbytes,
        in_registers: 16,
        returned: 16,
        large: Large::Stack,
        short: Short::Keeps,
        pair: Pair::Any,
        floats: Floats::Next,
        split: Split::Never,
        stack: Stack::Words,
        variadic: Variadic::FloatCount,
    };

    /// The rules a call of `signature` follows: a variadic one under [`Variadic::Core`] passes
    /// as the base standard does.
    #[must_use]
    pub fn calling(&self, signature: &vun_lir::Signature) -> Self {
        match (signature.variadic, self.variadic) {
            (Some(_), Variadic::Core) => Self { floats: Floats::Core, ..*self },
            _ => *self,
        }
    }

    /// The procedure call standard for arm64 (AAPCS64).
    pub const AAPCS64: Self = Self {
        word: 8,
        stack_align: 16,
        records: Records::Homogeneous { max: 4 },
        in_registers: 16,
        returned: 16,
        large: Large::Address,
        short: Short::Closes,
        pair: Pair::Even,
        floats: Floats::Next,
        split: Split::Never,
        stack: Stack::Words,
        variadic: Variadic::Fixed,
    };

    /// Apple's variant of AAPCS64, on macOS and iOS: a C call's scalars packed on the stack, a
    /// value of two words in any two general registers, and variadic arguments on the stack.
    pub const DARWIN_ARM64: Self =
        Self { stack: Stack::Packed, pair: Pair::Any, variadic: Variadic::Stack, ..Self::AAPCS64 };

    /// The procedure call standard for 32-bit Arm with floats in VFP registers (AAPCS, section
    /// 6.5, the hard-float EABI): a record of any size travels in words, in registers then on the
    /// stack, and only a record of a word comes back in one.
    pub const AAPCS_VFP: Self = Self {
        word: 4,
        stack_align: 8,
        records: Records::Homogeneous { max: 4 },
        in_registers: u64::MAX,
        returned: 4,
        large: Large::Stack,
        short: Short::Closes,
        pair: Pair::Even,
        floats: Floats::Backfill,
        split: Split::WhileStackEmpty,
        stack: Stack::Words,
        variadic: Variadic::Core,
    };

    /// AAPCS's base standard, floats in general registers (the soft-float EABI).
    pub const AAPCS: Self = Self { floats: Floats::Core, ..Self::AAPCS_VFP };

    /// The rules of the convention targets name `convention`, where one is built: the one place
    /// a name is given its rules, which each architecture's conventions read.
    #[must_use]
    pub const fn of(convention: vun_target::Convention) -> Option<&'static Self> {
        use vun_target::Convention as Named;
        match convention {
            Named::Sysv => Some(&Self::SYSV),
            Named::Aapcs64 => Some(&Self::AAPCS64),
            Named::DarwinArm64 => Some(&Self::DARWIN_ARM64),
            Named::AapcsVfp => Some(&Self::AAPCS_VFP),
            Named::Aapcs => Some(&Self::AAPCS),
            Named::Lp64d | Named::Ilp32 | Named::Wasm32 => None,
        }
    }
}
