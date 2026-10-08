//! What a backend can do, known without running it.

use vun_codec::{Decode, Encode};
use vun_lir_passes::legalize::Half;
use vun_target::{Floats, TargetSpec};

use crate::Emits;

/// What a backend does that a program may require of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Feature {
    /// Passes and returns every C record by value itself; a backend without it may refuse a
    /// function that does (`Missing::Record`, `Missing::Placement`), which an assisting backend
    /// compiles instead.
    CAbi,
    /// Native vectors; otherwise vectors are lowered to scalars.
    Vectors,
    TailCalls,
    /// Uses the aliasing facts of spec 05, section 8.
    NoAlias,
    /// Reports frame sizes, which `stack.max` requires.
    FrameSizes,
    DebugInfo,
    /// Defines and calls functions under the calling conventions they name (`#[abi.stdcall]`),
    /// not only the target's own.
    Conventions,
    /// Checks each function's frame before the stack moves to it (`StackCheck`), which the `stack`
    /// checks need.
    StackChecks,
    /// Compiles the functions another backend cannot express ([`Backend::assist`]).
    Assist,
    /// Writes blocks of inline assembly in the function holding them; otherwise the driver moves
    /// each into a function of its own first (`Outline`).
    InlineAsm,
    /// Takes the code an earlier build kept for a function ([`Request::kept`](crate::Request::kept))
    /// instead of compiling it; the driver legalizes only the functions it holds no code for.
    Keeps,
}

/// Capabilities a program may require of its backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Widest {
    /// Of this many bits, on every target.
    Bits(u32),
    /// A general register's, as wide as the target states.
    Register,
}

impl Widest {
    /// Its bits on `target`.
    #[must_use]
    pub fn on(self, target: &TargetSpec) -> u32 {
        match self {
            Self::Bits(bits) => bits,
            Self::Register => target.register_bits,
        }
    }
}

impl BackendCaps {
    #[must_use]
    pub fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}
