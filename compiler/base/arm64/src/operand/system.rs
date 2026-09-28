//! System registers and barriers.

/// A system register `mrs` reads and `msr` writes, by its encoding: `o0`, `op1`, `CRn`, `CRm` and
/// `op2` from bit 5 of the word, less the leading bit of `op0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SysReg {
    /// The condition flags.
    Nzcv = 0x5a10,
}

impl SysReg {
    pub const ALL: [Self; 1] = [Self::Nzcv];

    /// Its field, from bit 5.
    #[must_use]
    pub const fn field(self) -> u32 {
        self as u32
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nzcv => "nzcv",
        }
    }
}

/// What a memory barrier orders (`dmb`): accesses within a shareability domain, by its encoding
/// in `CRm`. The outer (`osh`), non-shareable (`nsh`) and inner (`ish`) domains, and the full
/// system (`sy`), each for loads (`ld`), stores (`st`) or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Barrier {
    Oshld = 1,
    Oshst = 2,
    Osh = 3,
    Nshld = 5,
    Nshst = 6,
    Nsh = 7,
    Ishld = 9,
    Ishst = 10,
    Ish = 11,
    Ld = 13,
    St = 14,
    Sy = 15,
}

impl Barrier {
    pub const ALL: [Self; 12] = [
        Self::Oshld,
        Self::Oshst,
        Self::Osh,
        Self::Nshld,
        Self::Nshst,
        Self::Nsh,
        Self::Ishld,
        Self::Ishst,
        Self::Ish,
        Self::Ld,
        Self::St,
        Self::Sy,
    ];

    /// Its field, `CRm`.
    #[must_use]
    pub const fn field(self) -> u32 {
        self as u32
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oshld => "oshld",
            Self::Oshst => "oshst",
            Self::Osh => "osh",
            Self::Nshld => "nshld",
            Self::Nshst => "nshst",
            Self::Nsh => "nsh",
            Self::Ishld => "ishld",
            Self::Ishst => "ishst",
            Self::Ish => "ish",
            Self::Ld => "ld",
            Self::St => "st",
            Self::Sy => "sy",
        }
    }
}
