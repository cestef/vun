//! Values a pass replaced, by what stands for them: a load by the value stored, a folded
//! operation by its operand. Chains resolve to their end. Values are dense, so they index a
//! table rather than hash.

use vun_index::Idx;

use vun_lir::entity::Value;

#[derive(Default)]
pub(crate) struct Replaced {
    by: Vec<Option<Value>>,
    count: usize,
}

impl Replaced {
    pub(crate) fn insert(&mut self, value: Value, by: Value) {
        debug_assert_ne!(value, by, "a value replaced by itself never resolves");
        if self.by.len() <= value.index() {
            self.by.resize(value.index() + 1, None);
        }
        if self.by[value.index()].replace(by).is_none() {
            self.count += 1;
        }
    }

    pub(crate) fn contains(&self, value: Value) -> bool {
        self.by.get(value.index()).is_some_and(Option::is_some)
    }

    /// How many values are replaced.
    pub(crate) fn len(&self) -> usize {
        self.count
    }

    /// What stands for `value` in the end.
    pub(crate) fn resolve(&self, mut value: Value) -> Value {
        while let Some(&Some(next)) = self.by.get(value.index()) {
            value = next;
        }
        value
    }
}
