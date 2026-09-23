//! Origin chains tracing every compiler entity back to source.
//!
//! Guarantee 5 of the language (spec 00): every token, syntax node, HIR node, MIR statement, LIR
//! instruction, artifact symbol and link-plan entry holds an [`OriginId`]. An [`Origin`] records a
//! span, how the entity came to exist ([`Via`]), and the origin it was derived from. Chains are
//! shared, so the cost per entity is one `u32`. See spec 10, section 1.

use overt_index::{IndexVec, index};
use overt_span::Span;

index! {
    /// Index of an [`Origin`] in an [`OriginTable`].
    pub struct OriginId;
}

/// Identifies one rewrite of the desugaring table (spec appendix B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DesugarRule(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PassId(pub u16);

/// How an entity came to exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The source range most directly responsible for the entity.
    pub span: Span,
    pub via: Via,
    pub parent: Option<OriginId>,
}

#[derive(Default)]
pub struct OriginTable {
    origins: IndexVec<OriginId, Origin>,
}

impl OriginTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an origin for user-written source.
    pub fn source(&mut self, span: Span) -> OriginId {
        self.push(Origin { span, via: Via::Source, parent: None })
    }

    /// Records an origin derived from `parent`.
    pub fn derive(&mut self, parent: OriginId, span: Span, via: Via) -> OriginId {
        self.push(Origin { span, via, parent: Some(parent) })
    }

    pub fn push(&mut self, origin: Origin) -> OriginId {
        self.origins.push(origin)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.origins.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    #[must_use]
    pub fn get(&self, id: OriginId) -> &Origin {
        &self.origins[id]
    }

    /// Walks from `id` towards user code, starting with `id` itself.
    #[must_use]
    pub fn chain(&self, id: OriginId) -> Chain<'_> {
        Chain { table: self, next: Some(id) }
    }

    /// The first user-written span in the chain of `id`, or the last span if there is none.
    #[must_use]
    pub fn source_span(&self, id: OriginId) -> Span {
        let mut last = self.get(id).span;
        for (_, origin) in self.chain(id) {
            if origin.via == Via::Source {
                return origin.span;
            }
            last = origin.span;
        }
        last
    }
}

/// Iterator over an origin chain; see [`OriginTable::chain`].
pub struct Chain<'a> {
    table: &'a OriginTable,
    next: Option<OriginId>,
}

impl<'a> Iterator for Chain<'a> {
    type Item = (OriginId, &'a Origin);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.next?;
        let origin = self.table.get(id);
        self.next = origin.parent;
        Some((id, origin))
    }
}

#[cfg(test)]
mod tests;
