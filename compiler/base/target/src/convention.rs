//! `lang.target.Convention`: how a target's C functions pass their arguments and results.

use std::fmt;

use vun_codec::{Decode, Encode};

/// `lang.target.Convention`: the calling convention of a target's C functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Convention {
    Sysv,
    Aapcs64,
    DarwinArm64,
    Win64,
    Lp64d,
    AapcsVfp,
    Aapcs,
    Ilp32,
    Wasm32,
    Cdecl,
    Stdcall,
    Fastcall,
    Thiscall,
}

impl Convention {
    /// The name `lang.target.Convention` gives it, which `#[abi.*]` names it by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sysv => "sysv",
            Self::Aapcs64 => "aapcs64",
            Self::DarwinArm64 => "darwin_arm64",
            Self::Win64 => "win64",
            Self::Lp64d => "lp64d",
            Self::AapcsVfp => "aapcs_vfp",
            Self::Aapcs => "aapcs",
            Self::Ilp32 => "ilp32",
            Self::Wasm32 => "wasm32",
            Self::Cdecl => "cdecl",
            Self::Stdcall => "stdcall",
            Self::Fastcall => "fastcall",
            Self::Thiscall => "thiscall",
        }
    }

    /// The architecture whose C functions follow it: a function of a target may be declared with
    /// the conventions of the target's own.
    #[must_use]
    pub const fn arch(self) -> &'static str {
        match self {
            Self::Sysv | Self::Win64 => "x86_64",
            Self::Aapcs64 | Self::DarwinArm64 => "arm64",
            Self::Lp64d => "riscv64",
            Self::AapcsVfp | Self::Aapcs => "arm",
            Self::Ilp32 => "riscv32",
            Self::Wasm32 => "wasm32",
            Self::Cdecl | Self::Stdcall | Self::Fastcall | Self::Thiscall => "x86",
        }
    }
}

impl fmt::Display for Convention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
