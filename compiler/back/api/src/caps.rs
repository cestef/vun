//! What a backend can do, known without running it.

use vun_lir_passes::legalize::Half;
use vun_target::{Floats, TargetSpec};

use crate::{Emits, Feature};

/// Capabilities a program may require of its backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendCaps {
    pub emits: Emits,
    pub features: &'static [Feature],
    /// The widest integer it computes with: wider ones are split into halves before it reads the
    /// unit (`Legalize`).
    pub widest: Widest,
    /// Whether it computes `f16`, or holds it as bits and computes in `f32` (`Unit::legalize`).
    pub half: Half,
    /// The floats it holds as floats; each other is held as its bits (`Legalize`), every
    /// operation on it having run as `lang.float.soft`'s.
    pub floats: HeldFloats,
}

/// The floats a backend holds as floats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldFloats {
    /// Every one: a compiler it hands its code to computes those the processor does not.
    Every,
    /// Those the target's processor computes.
    Computed,
}

impl HeldFloats {
    /// The floats it holds on `target`.
    #[must_use]
    pub fn on(self, target: &TargetSpec) -> Floats {
        match self {
            Self::Every => Floats::ALL,
            Self::Computed => target.floats,
        }
    }
}

/// The widest integer a backend computes with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widest {
    /// Of this many bits, on every target.
    Bits(u32),
    /// A register's: as wide as the target's addresses.
    Address,
}

impl Widest {
    /// Its bits on `target`.
    #[must_use]
    pub fn on(self, target: &TargetSpec) -> u32 {
        match self {
            Self::Bits(bits) => bits,
            Self::Address => target.pointer_bits,
        }
    }
}

impl BackendCaps {
    #[must_use]
    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}
