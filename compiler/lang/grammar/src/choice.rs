//! Choosing among kinds that may spell the same source: which is preferred (`preferred`), and
//! which kinds a field may hold (`parts`, `admits`, `holds`).

use std::sync::LazyLock;

use foldhash::HashMap;

use crate::{ADMITS, HOLDS, NodeField, NodeKind, PARTS, PREFERRED, TokenKind};

/// The kinds starting with each token, the preferred first.
static STARTING: LazyLock<HashMap<TokenKind, Vec<NodeKind>>> = LazyLock::new(|| {
    let mut starting: HashMap<TokenKind, Vec<NodeKind>> = HashMap::default();
    for &kind in NodeKind::ALL {
        for &token in kind.starts() {
            starting.entry(token).or_default().push(kind);
        }
    }
    for kinds in starting.values_mut() {
        kinds.sort_by_key(|k| k.rank());
    }
    starting
});

impl NodeKind {
    /// How many kinds are preferred to this one, one before another: the longest chain of kinds
    /// listed before it in rows of `preferred`.
    #[must_use]
    pub fn rank(self) -> usize {
        let before = PREFERRED.iter().filter_map(|row| Some(&row[..row.iter().position(|&k| k == self)?]));
        before.flatten().map(|k| k.rank() + 1).max().unwrap_or(0)
    }

    /// The kinds whose productions start with `token`, the preferred first.
    #[must_use]
    pub fn starting_with(token: TokenKind) -> &'static [Self] {
        STARTING.get(&token).map_or(&[], Vec::as_slice)
    }

    /// Whether the field `field` of this kind may hold a node of `kind`: a kind the field is
    /// named after, or where a row of `holds` names the field, a kind it lists; elsewhere any
    /// kind but a part, the parts a row of `admits` lists for the field, and those of `context`,
    /// the operand context read in. An `Error` fits any field.
    #[must_use]
    pub fn admits(self, field: NodeField, kind: Self, context: &[Self]) -> bool {
        let named = self.slots().iter().any(|s| s.field == field && s.kind == Some(kind));
        let mut held = HOLDS.iter().filter(|row| row.fields.contains(&field)).peekable();
        let admitted = || {
            !PARTS.contains(&kind)
                || context.contains(&kind)
                || ADMITS.iter().any(|row| row.fields.contains(&field) && row.kinds.contains(&kind))
        };
        kind == Self::Error
            || named
            || if held.peek().is_some() { held.any(|row| row.kinds.contains(&kind)) } else { admitted() }
    }
}
