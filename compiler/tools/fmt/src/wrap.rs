//! Lines that are too long (spec 13, section 5): a group on one that runs past `MAX_WIDTH` is
//! broken, one entry a line, outermost first; a line with no group to break is continued before
//! an infix operator. What is still too long is looked at again.

use std::collections::BTreeSet;

use vun_syntax::tokens::TokenId;

use crate::write::{Trace, Written};

/// The most columns a line has unless it cannot be broken: a string, a name, a comment.
pub const MAX_WIDTH: usize = 110;

/// Adds to `forced` the tokens to break before, so that what a pass left past `MAX_WIDTH` is
/// broken, a line at a time: the outermost group on it that holds the first token past the
/// width, or else its last infix operator that starts within it, of the fewest groups. Whether
/// any was added.
pub(crate) fn explode(trace: &Trace, forced: &mut BTreeSet<TokenId>) -> bool {
    let mut changed = false;
    let mut seen = BTreeSet::new();
    for over in trace.placed.iter().filter(|p| p.end > MAX_WIDTH) {
        if !seen.insert(over.line) {
            continue;
        }
        let on_line = |token: TokenId| trace.placed.iter().any(|p| p.token == token && p.line == over.line);
        let first = trace
            .placed
            .iter()
            .find(|p| p.line == over.line && p.end > MAX_WIDTH)
            .map_or(over.token, |p| p.token);
        let breakable = |g: &&Written| {
            !g.entries.is_empty() && g.entries.iter().chain(g.close.iter()).any(|t| !forced.contains(t))
        };
        let holds = |g: &&Written| on_line(g.open) && g.open < first && g.close.is_none_or(|c| c >= first);
        let group = trace.groups.iter().filter(breakable).find(holds);
        if let Some(group) = group {
            let group = hug(trace, group, forced);
            for &t in group.entries.iter().chain(group.close.iter()) {
                changed |= forced.insert(t);
            }
        } else if let Some(operator) = continued(trace, over.line, first, forced) {
            changed |= forced.insert(operator);
        }
    }
    changed
}

/// The group to break in place of `group`: when it holds one entry that is a group closing right
/// before it, as in `f(.{ ... })`, that one, so that the line reads `f(.{` and ends `})`.
fn hug<'t>(trace: &'t Trace, group: &'t Written, forced: &BTreeSet<TokenId>) -> &'t Written {
    let at = |token: TokenId| trace.placed.iter().position(|p| p.token == token);
    let mut group = group;
    while let [entry] = group.entries.as_slice() {
        let inner = trace.groups.iter().find(|g| g.open == *entry);
        let hugging = inner.filter(|g| {
            let (inside, outside) = (g.close.and_then(at), group.close.and_then(at));
            inside.zip(outside).is_some_and(|(inside, outside)| inside + 1 == outside)
                && !g.entries.is_empty()
                && g.entries.iter().chain(g.close.iter()).any(|t| !forced.contains(t))
        });
        match hugging {
            Some(inner) => group = inner,
            None => break,
        }
    }
    group
}

/// The infix operator to start a new line with: of those on `line` that stand outside the most
/// groups, the last one whose line would begin within the width.
fn continued(trace: &Trace, line: usize, first: TokenId, forced: &BTreeSet<TokenId>) -> Option<TokenId> {
    let position = |token: TokenId| trace.placed.iter().position(|p| p.token == token);
    let usable: Vec<(TokenId, usize)> = trace
        .infix
        .iter()
        .copied()
        .filter(|&(token, _)| !forced.contains(&token) && token < first)
        .filter(|&(token, _)| position(token).is_some_and(|i| trace.placed[i].line == line && i > 0))
        .filter(|&(token, _)| {
            position(token)
                .is_some_and(|i| trace.placed[i - 1].line == line && trace.placed[i - 1].end <= MAX_WIDTH)
        })
        .collect();
    let depth = usable.iter().map(|&(_, depth)| depth).min()?;
    usable.into_iter().rev().find(|&(_, d)| d == depth).map(|(token, _)| token)
}
