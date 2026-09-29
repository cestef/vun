//! Helper attributes, as `#[data(case = camel, untagged)]`: the arguments given under a helper's
//! name, each a key and an optional value of one token.

use proc_macro::{Delimiter, Group, Ident, TokenTree};

use crate::depth::{Depth, Tree};
use crate::item::Unsupported;

/// The attributes written on an item, a field or a variant.
#[derive(Clone, Default)]
pub struct Attrs(Vec<Group>);

/// One argument of a helper attribute: `key` or `key = value`.
#[derive(Clone)]
pub struct Arg {
    pub key: Ident,
    pub value: Option<TokenTree>,
}

impl Attrs {
    /// The attributes among `trees`: each `#` followed by a bracketed group.
    pub(crate) fn read<'t>(trees: impl IntoIterator<Item = &'t TokenTree>) -> Self {
        let mut attrs = Vec::new();
        let mut hash = false;
        for tree in trees {
            match tree {
                TokenTree::Group(g) if hash && g.delimiter() == Delimiter::Bracket => attrs.push(g.clone()),
                _ => {}
            }
            hash = tree.is('#');
        }
        Self(attrs)
    }

    /// The arguments of every `#[helper(...)]`, in order.
    ///
    /// # Errors
    /// An argument that is not a key, or `key = value` with a value of one token.
    pub fn args(&self, helper: &str) -> Result<Vec<Arg>, Unsupported> {
        let mut args = Vec::new();
        for attr in &self.0 {
            let trees: Vec<TokenTree> = attr.stream().into_iter().collect();
            let [name, TokenTree::Group(inner)] = trees.as_slice() else { continue };
            if !name.is_word(helper) || inner.delimiter() != Delimiter::Parenthesis {
                continue;
            }
            for arg in Depth::split(inner.stream().into_iter().collect(), false) {
                args.push(match arg.as_slice() {
                    [TokenTree::Ident(key)] => Arg { key: key.clone(), value: None },
                    [TokenTree::Ident(key), eq, value] if eq.is('=') => {
                        Arg { key: key.clone(), value: Some(value.clone()) }
                    }
                    _ => return Err(Unsupported("expected `key` or `key = value` in a helper attribute")),
                });
            }
        }
        Ok(args)
    }
}
