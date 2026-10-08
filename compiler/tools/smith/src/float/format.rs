//! The three formats, and the value each rounds an `f64` to.

/// An IEEE 754 binary format of the language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_variants::Variants)]
pub enum Format {
    F16,
    F32,
    F64,
}

impl Format {
    /// Bits of the significand, the hidden one included.
    #[must_use]
    pub const fn precision(self) -> u32 {
        match self {
            Self::F16 => 11,
            Self::F32 => 24,
            Self::F64 => 53,
        }
    }

    /// Bits of the encoding.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Self::F16 => 16,
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }

    /// The least exponent of a normal number.
    const fn least(self) -> i32 {
        match self {
            Self::F16 => -14,
            Self::F32 => -126,
            Self::F64 => -1022,
        }
    }

    /// The greatest finite number.
    #[must_use]
    pub fn greatest(self) -> f64 {
        match self {
            Self::F16 => 65504.0,
            Self::F32 => f64::from(f32::MAX),
            Self::F64 => f64::MAX,
        }
    }

    /// `value` rounded to this format, ties to even, overflowing to infinity.
    #[must_use]
    pub fn round(self, value: f64) -> f64 {
        match self {
            Self::F64 => value,
            Self::F32 => f64::from(value as f32),
            Self::F16 => {
                if !value.is_finite() || value == 0.0 {
                    return value;
                }
                let magnitude = value.abs();
                let exponent = (((magnitude.to_bits() >> 52) & 0x7ff) as i32 - 1023).max(self.least());
                let quantum = 2f64.powi(exponent - 10);
                let rounded = (magnitude / quantum).round_ties_even() * quantum;
                let rounded = if rounded > 65504.0 { f64::INFINITY } else { rounded };
                rounded.copysign(value)
            }
        }
    }

    /// The encoding of `value`, which this format holds: a NaN is the canonical one with the
    /// value's sign.
    #[must_use]
    pub fn encoding(self, value: f64) -> u64 {
        let negative = value.is_sign_negative();
        let sign = u64::from(negative) << (self.bits() - 1);
        match self {
            Self::F64 if value.is_nan() => sign | 0x7ff8_0000_0000_0000,
            Self::F64 => value.to_bits(),
            Self::F32 if value.is_nan() => sign | 0x7fc0_0000,
            Self::F32 => u64::from((value as f32).to_bits()),
            Self::F16 => {
                if value.is_nan() {
                    return sign | 0x7e00;
                }
                if value.is_infinite() {
                    return sign | 0x7c00;
                }
                let magnitude = value.abs();
                if magnitude == 0.0 {
                    return sign;
                }
                let exponent = ((magnitude.to_bits() >> 52) & 0x7ff) as i32 - 1023;
                if exponent < self.least() {
                    return sign | (magnitude / 2f64.powi(-24)) as u64;
                }
                let fraction = ((magnitude / 2f64.powi(exponent) - 1.0) * 1024.0) as u64;
                sign | (((exponent + 15) as u64) << 10) | fraction
            }
        }
    }
}
