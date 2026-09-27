//! Ordering nodes by their declared requirements: every node after the nodes it requires.

use std::collections::HashMap;
use std::hash::Hash;

/// Why nodes cannot be ordered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderError<K> {
    /// A node requires a key that no node has.
    Missing { node: K, requires: K },
    /// The requirements form a cycle through this node.
    Cycle(K),
    /// Two nodes have this key.
    Duplicate(K),
}

impl<K: std::fmt::Display> std::fmt::Display for OrderError<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { node, requires } => {
                write!(f, "`{node}` requires `{requires}`, which is not present")
            }
            Self::Cycle(node) => write!(f, "the requirements of `{node}` form a cycle"),
            Self::Duplicate(node) => write!(f, "`{node}` is present twice"),
        }
    }
}

impl<K: std::fmt::Debug + std::fmt::Display> std::error::Error for OrderError<K> {}

/// Nodes by key, each with the keys it requires.
pub struct Requirements<K> {
    keys: Vec<K>,
    requires: Vec<Vec<K>>,
}

impl<K> Default for Requirements<K> {
    fn default() -> Self {
        Self { keys: Vec::new(), requires: Vec::new() }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Visiting,
    Done,
}

impl<K: Clone + Eq + Hash> Requirements<K> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node, returning its index.
    pub fn add(&mut self, key: K, requires: impl IntoIterator<Item = K>) -> usize {
        self.keys.push(key);
        self.requires.push(requires.into_iter().collect());
        self.keys.len() - 1
    }

    /// Node indices, each after the nodes it requires, otherwise in the order they were added.
    ///
    /// # Errors
    /// Returns a missing requirement, a node on a cycle or a key two nodes have.
    pub fn order(&self) -> Result<Vec<usize>, OrderError<K>> {
        let mut index: HashMap<&K, usize> = HashMap::with_capacity(self.keys.len());
        for (i, key) in self.keys.iter().enumerate() {
            if index.insert(key, i).is_some() {
                return Err(OrderError::Duplicate(key.clone()));
            }
        }
        let mut marks = vec![Mark::New; self.keys.len()];
        let mut order = Vec::with_capacity(self.keys.len());
        for i in 0..self.keys.len() {
            self.visit(i, &index, &mut marks, &mut order)?;
        }
        Ok(order)
    }

    fn visit(
        &self,
        i: usize,
        index: &HashMap<&K, usize>,
        marks: &mut [Mark],
        order: &mut Vec<usize>,
    ) -> Result<(), OrderError<K>> {
        match marks[i] {
            Mark::Done => return Ok(()),
            Mark::Visiting => return Err(OrderError::Cycle(self.keys[i].clone())),
            Mark::New => {}
        }
        marks[i] = Mark::Visiting;
        for requires in &self.requires[i] {
            let missing = || OrderError::Missing { node: self.keys[i].clone(), requires: requires.clone() };
            let j = *index.get(requires).ok_or_else(missing)?;
            self.visit(j, index, marks, order)?;
        }
        marks[i] = Mark::Done;
        order.push(i);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
