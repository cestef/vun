//! What the productions imply, derived once for every reader. Each fact is derived from those
//! before it, in the order they need each other, and never through the lookups that read the
//! finished table: no fact is read while it is being derived.

use std::sync::LazyLock;

use crate::{ADMITS, NodeField, NodeKind, PARTS, TRANSPARENT, TokenKind};

use super::first::{Firsts, Opening};
use super::{Automaton, KindSet, Read};

pub(super) struct Derived {
    /// The automaton of each kind with a production, by the kind's index.
    pub(super) automata: Vec<Option<Automaton>>,
    /// The kinds that stand as operands: with a production, no part, no root and no `error`.
    pub(super) operands: KindSet,
    pub(super) firsts: Firsts,
    /// The kinds whose productions begin with an operand.
    pub(super) continuing: KindSet,
    /// For each kind, the kinds that may lead to it, and so on, itself included.
    pub(super) leads_to: Vec<KindSet>,
    /// For each field, the parts that may lead to what it admits.
    pub(super) field_leading: Vec<KindSet>,
    /// For each kind, what it may start with where no operand was read before it.
    pub(super) openings: Vec<Opening>,
    /// The kinds with productions, preferred first, and where each stands among them.
    pub(super) by_rank: Vec<NodeKind>,
    pub(super) ranked: Vec<usize>,
    /// Those of `by_rank` continuing an operand.
    pub(super) continuing_by_rank: Vec<NodeKind>,
    /// For each token, the kinds that may start alone with it, whatever its spelling, and those
    /// that may when it is spelled as one of their words.
    pub(super) opening: Vec<KindSet>,
    pub(super) worded: Vec<KindSet>,
    /// For each token, the transparent kinds written with it; and every transparent kind.
    pub(super) transparent_with: Vec<KindSet>,
    pub(super) transparent: KindSet,
}

pub(super) static DERIVED: LazyLock<Derived> = LazyLock::new(Derived::new);

impl Derived {
    fn new() -> Self {
        fn automaton(automata: &[Option<Automaton>], k: NodeKind) -> Option<&Automaton> {
            automata[k.index()].as_ref()
        }

        let mut automata: Vec<Option<Automaton>> =
            NodeKind::ALL.iter().map(|&k| k.production().map(|terms| Automaton::of(k, terms))).collect();

        let parts = KindSet::of(PARTS);
        let operand =
            |k: NodeKind| automaton(&automata, k).is_some_and(|a| !a.reads_end()) && !parts.contains(k);
        let operands: KindSet =
            NodeKind::ALL.iter().copied().filter(|&k| operand(k) && k != NodeKind::Error).collect();

        for a in automata.iter_mut().flatten() {
            let takes = |p: usize| matches!(&a.reads[p], Read::Child(c) if c.takes_operands_among(operands));
            a.leading = a.first.filter(takes);
        }
        let firsts = Firsts::new(&automata, operands);

        let leads = |k: NodeKind| {
            let Some(a) = automaton(&automata, k) else { return KindSet::EMPTY };
            a.leading.iter().fold(KindSet::EMPTY, |set, p| match &a.reads[p] {
                Read::Child(child) => set.union(child.admitted).union(child.kinds.unwrap_or_default()),
                _ => set,
            })
        };
        let leads_to: Vec<KindSet> = NodeKind::ALL
            .iter()
            .map(|&kind| {
                let mut set = KindSet::EMPTY.with(kind);
                loop {
                    let wider = set.iter().fold(set, |all, k| all.union(leads(k)));
                    if wider == set {
                        return set;
                    }
                    set = wider;
                }
            })
            .collect();
        let leading = |set: KindSet| set.iter().fold(set, |all, k| all.union(leads_to[k.index()]));
        for read in automata.iter_mut().flatten().flat_map(|a| a.reads.iter_mut()) {
            if let Read::Child(child) = read {
                child.leading = leading(child.kinds.unwrap_or_default());
            }
        }
        let admitted = |field: &NodeField| {
            let rows = ADMITS.iter().filter(|row| row.fields.contains(field));
            leading(rows.fold(KindSet::EMPTY, |set, row| set.union(KindSet::of(row.kinds))))
        };
        let field_leading = NodeField::ALL.iter().map(admitted).collect();

        let continuing: KindSet = NodeKind::ALL
            .iter()
            .copied()
            .filter(|&k| automaton(&automata, k).is_some_and(|a| !a.leading.is_empty()))
            .collect();
        let openings = Opening::all(&automata);

        let mut by_rank: Vec<NodeKind> =
            NodeKind::ALL.iter().copied().filter(|&k| automaton(&automata, k).is_some()).collect();
        by_rank.sort_by_key(|k| k.rank());
        let mut ranked = vec![usize::MAX; NodeKind::ALL.len()];
        for (at, kind) in by_rank.iter().enumerate() {
            ranked[kind.index()] = at;
        }
        let continuing_by_rank = by_rank.iter().copied().filter(|&k| continuing.contains(k)).collect();

        let opened = |by: &dyn Fn(&Opening, TokenKind) -> bool| -> Vec<KindSet> {
            let kinds =
                |token| NodeKind::ALL.iter().copied().filter(|k| by(&openings[k.index()], token)).collect();
            TokenKind::ALL.iter().map(|&token| kinds(token)).collect()
        };
        let opening = opened(&|o, token| o.tokens.contains(token));
        let worded = opened(&|o, token| o.words.iter().any(|word| word.tokens.contains(token)));
        let transparent_with = TokenKind::ALL
            .iter()
            .map(|token| {
                TRANSPARENT.iter().filter(|row| row.tokens.contains(token)).map(|row| row.kind).collect()
            })
            .collect();
        let transparent = TRANSPARENT.iter().map(|row| row.kind).collect();

        Self {
            automata,
            operands,
            firsts,
            continuing,
            leads_to,
            field_leading,
            openings,
            by_rank,
            ranked,
            continuing_by_rank,
            opening,
            worded,
            transparent_with,
            transparent,
        }
    }
}
