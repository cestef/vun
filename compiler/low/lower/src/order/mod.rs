//! The order a program's functions are inlined in: callees before the functions calling them.
//! Inlined together, each function would read its callees' inlined LIR from the thread that
//! asked for it, and threads asking for functions sharing a callee would wait on the one that
//! computes it. Each layer holds the functions calling only functions of the layers before it,
//! so that none waits on another within a layer; functions calling each other are counted as
//! calling the one reached first.

use vun_hash::Map;
use vun_index::Idx;
use vun_query::Cx;

use crate::inlined::Inlining;
use crate::piece::FunctionLir;

#[cfg(test)]
mod tests;

impl Inlining {
    /// `keys` in layers, each calling only functions of earlier layers or of its own.
    pub(crate) fn layers(cx: &Cx<'_>, keys: &[Self]) -> Vec<Vec<Self>> {
        let pieces: Vec<_> = keys.iter().map(|key| key.piece.clone()).collect();
        let lowered = cx.get_all::<FunctionLir>(&pieces);
        let index: Map<&Self, usize> = keys.iter().zip(0..).collect();
        let callees: Vec<Vec<usize>> = keys
            .iter()
            .zip(&lowered)
            .map(|(key, lir)| {
                let Ok(lir) = lir.as_ref() else { return Vec::new() };
                let mut called: Vec<_> = lir.function.callees().collect();
                called.sort_unstable();
                called.dedup();
                let named = called.into_iter().filter_map(|g| key.callee(&lir.refs.functions[g.index()]));
                named.filter_map(|callee| index.get(&callee).copied()).collect()
            })
            .collect();
        Self::layered(&callees).into_iter().fold(Vec::new(), |mut layers, (at, layer)| {
            layers.resize_with(layers.len().max(layer + 1), Vec::new);
            layers[layer].push(keys[at].clone());
            layers
        })
    }

    /// The layer of each function, given the functions each calls: one past the last of its
    /// callees', a callee reached again while its own are looked at counting as the first.
    fn layered(callees: &[Vec<usize>]) -> Vec<(usize, usize)> {
        let mut layer: Vec<Option<usize>> = vec![None; callees.len()];
        let mut looking = vec![false; callees.len()];
        for start in 0..callees.len() {
            // Each function being looked at, with how many of its callees are.
            let mut stack = vec![(start, 0)];
            while let Some(&(at, seen)) = stack.last() {
                if layer[at].is_some() {
                    stack.pop();
                    continue;
                }
                looking[at] = true;
                if let Some(&callee) = callees[at].get(seen) {
                    stack.last_mut().expect("a function").1 += 1;
                    if callee != at && layer[callee].is_none() && !looking[callee] {
                        stack.push((callee, 0));
                    }
                    continue;
                }
                let below = callees[at].iter().filter(|&&c| c != at).filter_map(|&c| layer[c]);
                layer[at] = Some(below.max().map_or(0, |last| last + 1));
                looking[at] = false;
                stack.pop();
            }
        }
        layer.into_iter().enumerate().filter_map(|(at, layer)| Some((at, layer?))).collect()
    }
}
