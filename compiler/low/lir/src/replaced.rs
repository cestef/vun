//! Values a pass replaced, by what stands for them: a load by the value stored, a folded
//! operation by its operand. Chains resolve to their end.

use std::collections::HashMap;

use crate::entity::Value;

#[derive(Default)]
pub(crate) struct Replaced(HashMap<Value, Value>);

impl Replaced {
    pub(crate) fn insert(&mut self, value: Value, by: Value) {
        self.0.insert(value, by);
    }

    pub(crate) fn contains(&self, value: Value) -> bool {
        self.0.contains_key(&value)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// What stands for `value` in the end.
    pub(crate) fn resolve(&self, mut value: Value) -> Value {
        while let Some(&next) = self.0.get(&value) {
            value = next;
        }
        value
    }
}
