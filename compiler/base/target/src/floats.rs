//! The floats a target's processor computes.

use vun_codec::{Decode, Encode};

/// `lang.target.Spec.floats`: the IEEE 754 binary formats a target's processor computes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Floats {
    /// Bit `k` for the format of `Floats::WIDTHS[k]` bits.
    formats: u8,
}

impl Floats {
    /// The widths of the formats, in bits.
    pub const WIDTHS: [u32; 3] = [16, 32, 64];
    /// Every format.
    pub const ALL: Self = Self { formats: (1 << Self::WIDTHS.len()) - 1 };

    /// These and the format of `bits` bits; `None` where no format is that wide.
    #[must_use]
    pub fn with(self, bits: u32) -> Option<Self> {
        let k = Self::WIDTHS.iter().position(|&w| w == bits)?;
        Some(Self { formats: self.formats | 1 << k })
    }

    /// Whether the processor computes floats of `bits` bits: a format listed, and `f16` through
    /// `f32`, whose results round back to the `f16` ones (spec 04, section 2).
    #[must_use]
    pub fn computes(self, bits: u32) -> bool {
        let listed =
            |w: u32| Self::WIDTHS.iter().position(|&x| x == w).is_some_and(|k| self.formats & 1 << k != 0);
        listed(bits) || (bits == 16 && listed(32))
    }
}
