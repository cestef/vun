//! 64-bit Arm (the ELF for the Arm 64-bit architecture).

numbered! {
    /// A relocation's kind (`R_AARCH64_*`).
    pub enum Reloc: u32 {
        Abs64 = 257,
        Abs32 = 258,
        Prel32 = 261,
        /// The page of the symbol's address relative to the field's (`adrp`).
        AdrPrelPgHi21 = 275,
        /// The symbol's address within its page, as an `add` reads it.
        AddAbsLo12Nc = 277,
        /// The same, as a load or store of 8, 16, 32 or 64 bits reads it, scaled by its size.
        Ldst8AbsLo12Nc = 278,
        CondBr19 = 280,
        Jump26 = 282,
        Call26 = 283,
        Ldst16AbsLo12Nc = 284,
        Ldst32AbsLo12Nc = 285,
        Ldst64AbsLo12Nc = 286,
        /// The page of the symbol's entry in the global offset table.
        AdrGotPage = 311,
        /// The entry's address within its page, as a 64-bit load reads it.
        Ld64GotLo12Nc = 312,
        /// The page of the symbol's TLS descriptor, its address within the page as a 64-bit load
        /// and as an `add` read it, and the call through it, which the field marks.
        TlsDescAdrPage21 = 562,
        TlsDescLd64Lo12 = 563,
        TlsDescAddLo12 = 564,
        TlsDescCall = 569,
    }
}
