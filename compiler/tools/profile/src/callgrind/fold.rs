//! Folding callgrind's functions into the profile's: recursion levels become one function, and
//! functions that call each other are given one inclusive cost between them.

use std::collections::HashMap;

use vun_intern::Symbol;

use super::cycles::Cycles;
use super::{Edge, Function, Profile, Share, Source};

/// The suffix callgrind gives a recursion level, `name'2`.
const LEVEL: char = '\'';

/// A function as callgrind counts it: one recursion level of one name.
pub(super) struct Node {
    pub name: Symbol,
    pub source: Source,
    pub own: u64,
    pub calls: u64,
}

impl Node {
    /// The name without its level, and the level (0 for the outermost call).
    fn level(&self) -> (Symbol, u32) {
        match self.name.as_str().rsplit_once(LEVEL) {
            Some((base, level)) => match level.parse() {
                Ok(level) => (Symbol::intern(base), level),
                Err(_) => (self.name, 0),
            },
            None => (self.name, 0),
        }
    }
}

/// A function's sums over its levels.
struct Group {
    name: Symbol,
    source: Source,
    own: u64,
    calls: u64,
    /// Of the outermost level, if it is no part of a cycle.
    inclusive: Option<u64>,
}

impl Profile {
    /// The profile of `nodes`, with `edges` between them by caller and callee, each with how often
    /// the call ran and what it cost, and `spent`, the instructions run in each source.
    ///
    /// Callgrind separates the levels of a recursion: the outermost call's cost holds the deeper
    /// ones, so a function's inclusive cost is its outermost level's. A function that reaches
    /// itself through others has none: what the cycle ran is no cost of any member, and in a
    /// program whose parts call back into one engine most functions are in one cycle.
    pub(super) fn folded(
        nodes: &[Node],
        edges: &HashMap<(usize, usize), (u64, u64)>,
        spent: HashMap<Source, u64>,
    ) -> Self {
        let cycles = Cycles::of(nodes.len(), edges.keys().copied());
        let mut ran = vec![0; cycles.count];
        for (at, node) in nodes.iter().enumerate() {
            ran[cycles.of_function(at)] += node.own;
        }
        for (&(caller, callee), &(_, cost)) in edges {
            if !cycles.together(caller, callee) {
                ran[cycles.of_function(caller)] += cost;
            }
        }
        let mut index: HashMap<Symbol, usize> = HashMap::new();
        let mut groups: Vec<Group> = Vec::new();
        let group_of: Vec<usize> = nodes
            .iter()
            .enumerate()
            .map(|(at, node)| {
                let (name, _) = node.level();
                let group = *index.entry(name).or_insert_with(|| {
                    groups.push(Group { name, source: node.source, own: 0, calls: 0, inclusive: None });
                    groups.len() - 1
                });
                let sums = &mut groups[group];
                sums.own += node.own;
                sums.calls += node.calls;
                if node.level().1 == 0 && !cycles.is_cyclic(at) {
                    sums.inclusive = Some(ran[cycles.of_function(at)]);
                }
                group
            })
            .collect();
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by(|&a, &b| groups[b].own.cmp(&groups[a].own).then(a.cmp(&b)));
        let mut rank = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            rank[old] = new;
        }
        let functions: Vec<Function> = order
            .iter()
            .map(|&at| {
                let Group { name, source, own, calls, inclusive } = groups[at];
                Function { name, source, own, inclusive, calls }
            })
            .collect();
        // A call's cost is counted once, where the callee is the outermost level of its function.
        let mut folded: HashMap<(usize, usize), (u64, Option<u64>)> = HashMap::new();
        for (&(caller, callee), &(calls, cost)) in edges {
            let (from, to) = (rank[group_of[caller]], rank[group_of[callee]]);
            if from == to {
                continue;
            }
            let entry = folded.entry((from, to)).or_default();
            entry.0 += calls;
            if nodes[callee].level().1 == 0 && !cycles.together(caller, callee) {
                *entry.1.get_or_insert(0) += cost;
            }
        }
        let mut edges: Vec<Edge> = folded
            .into_iter()
            .map(|((caller, callee), (calls, cost))| Edge { caller, callee, calls, cost })
            .collect();
        edges.sort_by(|a, b| {
            b.cost
                .cmp(&a.cost)
                .then(b.calls.cmp(&a.calls))
                .then((a.caller, a.callee).cmp(&(b.caller, b.callee)))
        });
        let mut sources: Vec<Share> =
            spent.into_iter().map(|(source, instructions)| Share { source, instructions }).collect();
        sources.sort_by(|a, b| b.instructions.cmp(&a.instructions).then(a.source.cmp(&b.source)));
        Self { total: functions.iter().map(|f| f.own).sum(), functions, sources, edges }
    }
}
