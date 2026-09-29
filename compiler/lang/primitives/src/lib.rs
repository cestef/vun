//! The declarations of `lang` whose value is `_`, provided by the compiler, generated from
//! `lib/lang`: a module per namespace with an enum of its primitives, and `Primitive` over them.

#[rustfmt::skip]
mod generated;

use std::sync::LazyLock;

pub use generated::*;

impl Primitive {
    /// The primitive `item` is, as tables name them: `lang.mem.Ref`.
    #[must_use]
    pub fn at(item: vun_items::Item) -> Option<Self> {
        static BY_ITEM: LazyLock<foldhash::HashMap<vun_items::Item, Primitive>> =
            LazyLock::new(|| Primitive::all().map(|p| (p.item(), p)).collect());
        BY_ITEM.get(&item).copied()
    }
}

#[cfg(test)]
mod tests;
