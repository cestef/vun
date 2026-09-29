//! The arguments of a derive's helper attribute on one item, variant or field, read by key: each
//! key the derive declares with its name and where it may be written.

use proc_macro::TokenTree;

use crate::attr::{Arg, Attrs};
use crate::item::Unsupported;

/// Where an attribute is written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum On {
    Item,
    Variant,
    Field,
}

/// A key a derive declares: its value, its name in the attribute, and where it is allowed.
pub type Key<K> = (K, &'static str, &'static [On]);

/// The arguments given on one attribute list, by key.
pub struct Given<K>(Vec<(K, Option<TokenTree>)>);

impl<K: Copy + PartialEq> Given<K> {
    /// The arguments of every `#[helper(...)]` in `attrs`, written `on`.
    ///
    /// # Errors
    /// An argument no key names, or one not allowed where it is written.
    pub fn of(attrs: &Attrs, helper: &str, keys: &[Key<K>], on: On) -> Result<Self, Unsupported> {
        let keyed = attrs.args(helper)?.into_iter().map(|Arg { key, value }| {
            let key = key.to_string();
            let &(key, _, allowed) = keys
                .iter()
                .find(|(_, name, _)| *name == key)
                .ok_or(Unsupported("unknown helper argument"))?;
            if !allowed.contains(&on) {
                return Err(Unsupported("this helper argument is not allowed here"));
            }
            Ok((key, value))
        });
        keyed.collect::<Result<_, _>>().map(Self)
    }

    #[must_use]
    pub fn has(&self, key: K) -> bool {
        self.0.iter().any(|(k, _)| *k == key)
    }

    #[must_use]
    pub fn value(&self, key: K) -> Option<&TokenTree> {
        self.0.iter().find(|(k, _)| *k == key)?.1.as_ref()
    }

    /// The name `key = "..."` gives, or `own` in `casing`.
    ///
    /// # Errors
    /// A value that is not a plain string literal.
    pub fn name(&self, key: K, own: &str, casing: crate::Casing) -> Result<String, Unsupported> {
        let Some(given) = self.value(key) else { return Ok(casing.apply(own)) };
        let text = given.to_string();
        let name = text.strip_prefix('"').and_then(|t| t.strip_suffix('"'));
        name.filter(|n| !n.contains('\\'))
            .map(str::to_owned)
            .ok_or(Unsupported("a name is a plain string literal"))
    }
}
