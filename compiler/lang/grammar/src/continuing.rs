//! Forms continuing an operand: those whose productions begin with a child, the operand read
//! before them, and go on with tokens telling them apart.

use std::sync::LazyLock;

use foldhash::HashMap;

use crate::{Level, NodeKind, NodeTerm, PARTS, TokenKind};

/// A form whose production begins with the operand it continues.
#[derive(Clone, Debug)]
pub struct Continuation {
    pub kind: NodeKind,
    /// Its level, the operator's for the infix form.
    pub level: Option<Level>,
    /// The kinds the operand may be; `None` for any.
    operand: Option<&'static [NodeKind]>,
    /// The ways the rest may begin: the token and word terms before its first child, one run per
    /// alternative of a leading choice.
    pub leads: Vec<&'static [NodeTerm]>,
}

/// The forms continuing an operand, by the tokens their leading runs may begin with.
static BY_TOKEN: LazyLock<HashMap<TokenKind, Vec<Continuation>>> = LazyLock::new(|| {
    let mut by_token: HashMap<TokenKind, Vec<Continuation>> = HashMap::default();
    for c in ALL.iter() {
        let mut firsts: Vec<TokenKind> =
            c.leads.iter().filter_map(|run| run.first()).flat_map(first_tokens).collect();
        firsts.sort_by_key(|&t| t as usize);
        firsts.dedup();
        for token in firsts {
            by_token.entry(token).or_default().push(c.clone());
        }
    }
    by_token
});

/// The tokens a token or word term may be: for a word, a name or any keyword spelled so.
fn first_tokens(term: &NodeTerm) -> Vec<TokenKind> {
    match *term {
        NodeTerm::Token(kinds) => kinds.to_vec(),
        NodeTerm::Word(_) => {
            std::iter::once(TokenKind::Ident).chain(TokenKind::KEYWORDS.iter().copied()).collect()
        }
        _ => Vec::new(),
    }
}

static ALL: LazyLock<Vec<Continuation>> = LazyLock::new(|| {
    let continuation = |kind: NodeKind| {
        let (first, rest) = kind.production()?.split_first()?;
        let operand = match *first {
            NodeTerm::Field(_) => None,
            NodeTerm::Of(_, kinds) => Some(kinds),
            _ => return None,
        };
        Some(Continuation { kind, level: kind.level(), operand, leads: leads(rest) })
    };
    NodeKind::ALL.iter().copied().filter(|k| !PARTS.contains(k)).filter_map(continuation).collect()
});

impl Continuation {
    /// Every form continuing an operand: kinds that are operands themselves (no part), whose
    /// productions begin with a child any kind or the kinds listed may fill.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &ALL
    }

    /// The forms whose leading tokens may begin with `token`.
    #[must_use]
    pub fn at(token: TokenKind) -> &'static [Self] {
        BY_TOKEN.get(&token).map_or(&[], Vec::as_slice)
    }

    /// Whether the operand may be a node of `kind`.
    #[must_use]
    pub fn holds(&self, kind: NodeKind) -> bool {
        self.operand.is_none_or(|kinds| kinds.contains(&kind))
    }
}

/// The runs of token and word terms `terms` may begin with.
fn leads(terms: &'static [NodeTerm]) -> Vec<&'static [NodeTerm]> {
    if let Some(NodeTerm::Choice(alternatives)) = terms.first() {
        return alternatives
            .iter()
            .flat_map(|alternative| match alternative {
                NodeTerm::Sequence(terms) => leads(terms),
                term => leads(std::slice::from_ref(term)),
            })
            .collect();
    }
    let run = terms.iter().take_while(|t| matches!(t, NodeTerm::Token(_) | NodeTerm::Word(_))).count();
    vec![&terms[..run]]
}
