//! How an origin came to exist: from source, or made from another origin by a rewrite.

use vun_codec::{Decode, Encode};

use super::{DesugarRule, OriginId, PassId};

/// How an entity came to exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Via {
    Source,
    /// Produced by a desugaring rewrite.
    Desugar(DesugarRule),
    /// A body with compile-time parameters, instantiated here.
    Instantiate {
        generic: OriginId,
    },
    /// Code from a quote or template, spliced here.
    Splice {
        quote: OriginId,
    },
    /// A name produced by `$(...)` in a name position.
    ComputedName {
        producer: OriginId,
    },
    Lower(PassId),
    /// A callee's code inlined into the function declared at `into`, which the parent is the
    /// code of (spec 13, section 3).
    Inline {
        into: OriginId,
    },
    /// A part of its parent's source, such as a run of the bytes of a string literal that
    /// compile-time code points at (`lang.meta.within`).
    Part,
    /// A further entity made from the same source as its parent, the `n`-th: the origin of each
    /// entity is its own, however many come from one piece of syntax.
    Again(u16),
}

impl Via {
    /// Whether an origin made this way names source as the user wrote it: the source itself, or
    /// a part of it.
    #[must_use]
    pub fn written(self) -> bool {
        matches!(self, Self::Source | Self::Part)
    }
}
