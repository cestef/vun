//! 32-bit Arm (the ELF for the Arm architecture).

/// The version of the Arm EABI a file follows, in its flags.
pub const EABI5: u32 = 0x0500_0000;

numbered! {
    /// Where a file's functions pass floats, in its flags (`EF_ARM_ABI_FLOAT_*`).
    pub enum FloatAbi: u32 {
        /// In general registers.
        Soft = 0x200,
        /// In floating-point registers.
        Hard = 0x400,
    }
}

numbered! {
    /// A relocation's kind (`R_ARM_*`).
    pub enum Reloc: u32 {
        Abs32 = 2,
        Rel32 = 3,
        /// A Thumb `bl`'s target.
        ThmCall = 10,
        /// A Thumb `b.w`'s target.
        ThmJump24 = 30,
        /// The low and high halves of the symbol's address, for `movw` and `movt`.
        ThmMovwAbsNc = 47,
        ThmMovtAbs = 48,
        /// A Thumb `b<c>.w`'s target.
        ThmJump19 = 51,
    }
}

/// The names of the symbols marking what follows them, which the ABI fixes.
pub const THUMB: &[u8] = b"$t";
pub const DATA: &[u8] = b"$d";
