//! The language features a package can enable (spec 12, section 2).

use vun_codec::{Decode, Encode};
use vun_variants::Variants;

use crate::Chosen;

/// `lang.gate.Feature`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, Variants)]
pub enum Feature {
    /// `#[comptime.deferred]`.
    Deferred,
}

impl Feature {
    /// The name a manifest gives it: the variant's in `lang.gate.Feature`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deferred => "deferred",
        }
    }
}

/// The features a package enables.
pub type Features = Chosen<Feature>;
