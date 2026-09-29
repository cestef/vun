//! An instruction set's operand widths, as code generation over any of them reads them.

/// An instruction set's operand sizes.
pub trait Width: Copy {
    const B32: Self;
    const B64: Self;
    /// The size of `bytes`, a power of two up to 8.
    fn of_bytes(bytes: u64) -> Option<Self>;
    fn bits(self) -> u32;
}
