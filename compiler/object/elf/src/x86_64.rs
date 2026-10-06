//! x86-64 (the System V ABI's AMD64 supplement).

numbered! {
    /// A relocation's kind (`R_X86_64_*`).
    pub enum Reloc: u32 {
        /// The symbol's address, 64 bits.
        Abs64 = 1,
        /// The symbol's address relative to the field, 32 bits.
        Pc32 = 2,
        /// A call's target relative to the field, through the procedure linkage table if needed.
        Plt32 = 4,
        /// The symbol's entry in the global offset table, relative to the field.
        GotPcRel = 9,
        /// The symbol's address, 32 bits, zero-extended.
        Abs32 = 10,
        /// The symbol's entries for the general dynamic model, relative to the field.
        TlsGd = 19,
        /// The symbol's TLS descriptor in the global offset table, relative to the field.
        GotPc32TlsDesc = 34,
        /// A call through the symbol's TLS descriptor: marks the instruction.
        TlsDescCall = 35,
    }
}
