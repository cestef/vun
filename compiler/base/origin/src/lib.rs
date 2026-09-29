//! Origin chains tracing every compiler entity back to source.
//!
//! Guarantee 5 of the language (spec 00): every token, syntax node, HIR node, MIR statement, LIR
//! instruction, artifact symbol and link-plan entry holds an [`OriginId`]. An [`Origin`] records a
//! span, how the entity came to exist ([`Via`]), and the origin it was derived from. Chains are
//! shared, so the cost per entity is one `u32`. See spec 10, section 1.
//!
//! The table is append-only and shared: threads record origins through `&self` without waiting on
//! each other.
//!
//! An origin is anchored so that it is identified the same way in every process and across edits
//! elsewhere in its file: a declaration by its name inside the declaration around it, a node inside
//! a declaration by its span relative to the declaration, and what precedes declarations (tokens,
//! syntax errors) by its span in the file. [`OriginId`] hashes by that identity, computed once
//! when the origin is recorded, so hashes of anything holding origins survive both. A result
//! reused across such an edit holds origins recorded in the earlier text; [`Resolve`] finds them
//! in the text a compilation reads now.

use std::hash::{Hash, Hasher};

mod codec;
mod resolve;

pub use resolve::Resolve;

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_span::{FileId, Span};

/// An [`Origin`] in an [`OriginTable`], found by its position in the table, and equal and hashed
/// by the origin's anchored identity: the origin recorded again, as when read from another
/// process, is the same origin.
#[derive(Clone, Copy, Debug)]
pub struct OriginId {
    index: u32,
    identity: u64,
}

impl PartialEq for OriginId {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for OriginId {}

impl Hash for OriginId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.identity);
    }
}

/// What an origin's span is identified relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Anchor {
    /// The file: tokens and syntax, before declarations exist.
    File,
    /// A declaration, by its name inside the declaration around it, if any.
    Declaration { within: Option<OriginId>, name: Symbol },
    /// Inside this declaration, by the span relative to the declaration's start.
    Within(OriginId),
}

/// Identifies one rewrite of the desugaring table (spec appendix B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct DesugarRule(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct PassId(pub u16);

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
    /// A further entity made from the same source as its parent, the `n`-th: the origin of each
    /// entity is its own, however many come from one piece of syntax.
    Again(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Origin {
    /// The source range most directly responsible for the entity.
    pub span: Span,
    pub via: Via,
    pub parent: Option<OriginId>,
    pub anchor: Anchor,
}

#[derive(Default)]
pub struct OriginTable {
    origins: boxcar::Vec<Origin>,
    /// Each declaration read from source, by its identity and the file text it was read in.
    declared: papaya::HashMap<(u64, FileId), u32>,
}

impl OriginTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The origins of the process, which origin handles in interned values and saved memos are
    /// into, shared by every compilation in it.
    #[must_use]
    pub fn global() -> &'static Self {
        static ORIGINS: std::sync::LazyLock<OriginTable> = std::sync::LazyLock::new(OriginTable::new);
        &ORIGINS
    }

    /// Records an origin for user-written source identified by its place in the file.
    pub fn source(&self, span: Span) -> OriginId {
        self.push(Origin { span, via: Via::Source, parent: None, anchor: Anchor::File })
    }

    /// Records the origin of the declaration `name`, inside the declaration `within` if any.
    pub fn declaration(&self, span: Span, within: Option<OriginId>, name: Symbol) -> OriginId {
        let id = self.push(Origin {
            span,
            via: Via::Source,
            parent: None,
            anchor: Anchor::Declaration { within, name },
        });
        self.declared.pin().insert((id.identity, span.file), id.index);
        id
    }

    /// Records the origin of user-written source inside the declaration `declaration`.
    pub fn inside(&self, declaration: OriginId, span: Span) -> OriginId {
        self.push(Origin { span, via: Via::Source, parent: None, anchor: Anchor::Within(declaration) })
    }

    /// Records an origin derived from `parent`, anchored where `parent` is.
    pub fn derive(&self, parent: OriginId, span: Span, via: Via) -> OriginId {
        let anchor = match self.get(parent).anchor {
            Anchor::Declaration { .. } => Anchor::Within(parent),
            anchor => anchor,
        };
        self.push(Origin { span, via, parent: Some(parent), anchor })
    }

    /// # Panics
    /// Panics past `u32::MAX` origins.
    pub fn push(&self, origin: Origin) -> OriginId {
        let identity = self.identity(&origin);
        vun_trace::tracing::trace!(identity, ?origin, "origin");
        let index = u32::try_from(self.origins.push(origin)).expect("fewer than 2^32 origins");
        OriginId { index, identity }
    }

    /// The hash identifying `origin` in every process: how it came to exist, and its span relative
    /// to its anchor.
    fn identity(&self, origin: &Origin) -> u64 {
        let mut hasher = vun_hash::Stable::hasher();
        origin.via.hash(&mut hasher);
        origin.parent.hash(&mut hasher);
        origin.anchor.hash(&mut hasher);
        match origin.anchor {
            Anchor::File => origin.span.hash(&mut hasher),
            Anchor::Declaration { .. } => origin.span.file.hash(&mut hasher),
            Anchor::Within(declaration) => {
                let anchor = self.get(declaration).span;
                let offset = origin.span.range.lo.checked_sub(anchor.range.lo);
                // The same file under any id a session gave it: identities hold in every process.
                match offset.filter(|_| anchor.file.key() == origin.span.file.key()) {
                    Some(offset) => (offset, origin.span.range.len()).hash(&mut hasher),
                    // A span outside its anchor is named where it is, as a file's origin is.
                    None => origin.span.hash(&mut hasher),
                }
            }
        }
        hasher.finish()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.origins.count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    /// Whether `id` was recorded in this table.
    #[must_use]
    pub fn contains(&self, id: OriginId) -> bool {
        self.origins.get(id.index as usize).is_some()
    }

    /// # Panics
    /// Panics if `id` was not recorded in this table.
    #[must_use]
    pub fn get(&self, id: OriginId) -> &Origin {
        &self.origins[id.index as usize]
    }

    /// The name of the declaration `id` declares, or of the one it is written inside.
    #[must_use]
    pub fn declaration_name(&self, id: OriginId) -> Option<Symbol> {
        match self.get(id).anchor {
            Anchor::Declaration { name, .. } => Some(name),
            Anchor::Within(declaration) => self.declaration_name(declaration),
            Anchor::File => None,
        }
    }

    /// The names of the declaration `id` declares or is written inside and of those around it,
    /// outermost first: `Pair.swap` for a function `swap` declared in the struct `Pair`.
    #[must_use]
    pub fn declaration_path(&self, id: OriginId) -> Vec<Symbol> {
        let mut path = Vec::new();
        let mut at = Some(id);
        while let Some(id) = at {
            at = match self.get(id).anchor {
                Anchor::Declaration { within, name } => {
                    path.push(name);
                    within
                }
                Anchor::Within(declaration) => Some(declaration),
                Anchor::File => None,
            };
        }
        path.reverse();
        path
    }

    /// Walks from `id` towards user code, starting with `id` itself.
    #[must_use]
    pub fn chain(&self, id: OriginId) -> Chain<'_> {
        Chain { table: self, next: Some(id) }
    }

    /// The first user-written span in the chain of `id`, or the last span if there is none, as
    /// recorded; [`Resolve::source_span`] finds it in the text read now.
    #[must_use]
    pub fn source_span(&self, id: OriginId) -> Span {
        self.get(self.written(id)).span
    }

    /// The first user-written origin in the chain of `id`, or the last one if there is none.
    #[must_use]
    pub fn written(&self, id: OriginId) -> OriginId {
        let mut last = id;
        for (at, origin) in self.chain(id) {
            if origin.via == Via::Source {
                return at;
            }
            last = at;
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
