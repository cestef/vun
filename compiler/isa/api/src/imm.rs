//! Immediates: which values an encoding of some width holds, and the bits it holds them as.
//! Values are the bits of an `i64`; an operation of `op_bits` reads only its low `op_bits`.

/// The low 64 bits of a constant's bits, which every immediate is taken from.
#[must_use]
#[expect(clippy::cast_possible_truncation, reason = "the low 64 bits")]
pub const fn low64(bits: u128) -> u64 {
    bits as u64
}

/// The low `bits` bits of `value`.
#[must_use]
pub const fn truncate(value: i64, bits: u32) -> u64 {
    if bits >= 64 { value.cast_unsigned() } else { value.cast_unsigned() & ((1 << bits) - 1) }
}

/// The `bits` bits of `value` from bit `at`, as a field of an instruction word.
#[must_use]
#[expect(clippy::cast_possible_truncation, reason = "a field is narrower than a word")]
pub const fn field(value: i64, at: u32, bits: u32) -> u32 {
    debug_assert!(bits <= u32::BITS, "a field is narrower than a word");
    truncate(value >> at, bits) as u32
}

/// The low `bits` bits of `value`, sign-extended to 64: 0 of none.
#[must_use]
pub const fn sign_extend(value: u64, bits: u32) -> i64 {
    if bits == 0 {
        0
    } else if bits >= 64 {
        value.cast_signed()
    } else {
        let shift = 64 - bits;
        (value << shift).cast_signed() >> shift
    }
}

/// Whether an immediate of `imm_bits`, sign-extended to `op_bits`, gives `value` as an
/// operation of `op_bits` reads it.
#[must_use]
pub const fn fits_signed(value: i64, imm_bits: u32, op_bits: u32) -> bool {
    let wanted = truncate(value, op_bits);
    let held = truncate(sign_extend(truncate(value, imm_bits), imm_bits), op_bits);
    wanted == held
}

/// Whether an immediate of `imm_bits`, zero-extended to `op_bits`, gives `value` as an
/// operation of `op_bits` reads it.
#[must_use]
pub const fn fits_unsigned(value: i64, imm_bits: u32, op_bits: u32) -> bool {
    truncate(value, op_bits) == truncate(value, if imm_bits < op_bits { imm_bits } else { op_bits })
}

/// Whether an operation of `bits` holds `value`, read as signed or as unsigned: a constant a
/// block writes is never cut to the bits the operation reads.
#[must_use]
pub const fn held(value: i64, bits: u32) -> bool {
    bits >= 64 || (value >= -(1 << (bits - 1)) && value < (1 << bits))
}
