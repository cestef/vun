//! What a position takes as its child: the one thing the reader consults to read a child, and to
//! tell whether what it read may stand there.

use std::sync::LazyLock;

use vun_grammar::{Child, KindSet, Level, NodeKind, OPERAND_CONTEXTS, TRANSPARENT, TokenSet};

/// The kinds of `transparent`.
static TRANSPARENT_KINDS: LazyLock<KindSet> =
    LazyLock::new(|| KindSet::of(&TRANSPARENT.iter().map(|row| row.kind).collect::<Vec<_>>()));

/// What one or more positions take as their child, merged.
#[derive(Clone, Copy, Debug)]
pub(super) struct Spec {
    /// Whether it may be any operand.
    pub(super) free: bool,
    /// The kinds positions name.
    pub(super) named: KindSet,
    /// Whether a transparent node may stand for one of `named` (`.of`, `holds`).
    pub(super) through: bool,
    /// The parts it may be (`admits`).
    pub(super) admits: KindSet,
    /// Those, and the parts that may lead to them.
    pub(super) leading: KindSet,
    /// The tokens it must start with, where every position says so.
    pub(super) starting: Option<TokenSet>,
    /// How tightly it holds the forms continuing it.
    pub(super) level: Level,
    /// The kinds `operand_contexts` makes operands here, in place of those starting with the same
    /// tokens, inherited by everything read inside.
    pub(super) context: KindSet,
}

impl Spec {
    /// What a position reading `child` takes at `level`, read in `context`.
    pub(super) fn child(child: &Child, level: Level, context: KindSet) -> Self {
        let rows = OPERAND_CONTEXTS.iter().filter(|row| row.fields.contains(&child.field));
        Self {
            free: child.kinds.is_none(),
            named: child.kinds.unwrap_or_default(),
            through: child.through(),
            admits: child.admitted,
            leading: child.field.leading(),
            starting: child.starting,
            level,
            context: rows.fold(context, |set, row| set.union(KindSet::of(row.kinds))),
        }
    }

    /// What either takes, at the tighter level: a child goes on as a form only where every
    /// position reading it lets it.
    pub(super) fn union(self, other: Self) -> Self {
        Self {
            free: self.free || other.free,
            named: self.named.union(other.named),
            through: self.through || other.through,
            admits: self.admits.union(other.admits),
            leading: self.leading.union(other.leading),
            starting: self.starting.zip(other.starting).map(|(a, b)| a.union(b)),
            level: self.level.max(other.level),
            context: self.context.union(other.context),
        }
    }

    /// The same, at `level`.
    pub(super) fn at(self, level: Level) -> Self {
        Self { level, ..self }
    }

    /// Whether a node of `kind` may stand here as it is; an `Error` stands anywhere.
    pub(super) fn holds(&self, kind: NodeKind) -> bool {
        self.held().contains(kind)
    }

    /// The kinds a node may stand here as: those named, of the context, an `Error`, and where it
    /// is free, operands and the parts admitted.
    fn held(&self) -> KindSet {
        let named = self.named.union(self.context).with(NodeKind::Error);
        if self.free { named.union(NodeKind::operands()).union(self.admits) } else { named }
    }

    /// Whether a node of one of `kinds` may stand here, as it is or, where transparent nodes stand
    /// for what they end with, as one ending with one of `ends`.
    pub(super) fn may_hold(&self, kinds: KindSet, ends: KindSet) -> bool {
        let held = self.held();
        !held.intersection(kinds).is_empty() || (self.through && !held.intersection(ends).is_empty())
    }

    /// Whether it may be an operand, which a token the lexer reported may stand for.
    pub(super) fn takes_operands(&self) -> bool {
        self.free || !self.named.intersection(NodeKind::operands()).is_empty()
    }

    /// The kinds a node read here may be on the way to what stands here: those named and the
    /// parts admitted, what leads to them, and where it is free or a named kind continues an
    /// operand, any operand.
    pub(super) fn reachable(&self) -> KindSet {
        let continuing = self.named.iter().any(NodeKind::continues);
        let named = if continuing { self.named.leading() } else { self.named };
        let parts = named.union(self.leading);
        if self.free || continuing { parts.union(NodeKind::operands()).union(self.context) } else { parts }
    }

    /// The transparent kinds that may stand for a kind named, where nothing else reaches them.
    pub(super) fn standing_for(&self) -> KindSet {
        if self.through { TRANSPARENT_KINDS.without(self.reachable()) } else { KindSet::EMPTY }
    }
}
