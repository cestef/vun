//! What the host answered about each origin, which does not change while a call runs: asking
//! again would encode the origin and cross the boundary for what the backend already has.

use std::cell::RefCell;
use std::collections::HashMap;

use vun_origin::OriginId;

pub(crate) struct Memo<V>(RefCell<HashMap<OriginId, V>>);

impl<V> Default for Memo<V> {
    fn default() -> Self {
        Self(RefCell::default())
    }
}

impl<V: Clone> Memo<V> {
    /// The answer for `origin`, fetched the first time.
    pub(crate) fn get(&self, origin: OriginId, fetch: impl FnOnce() -> V) -> V {
        if let Some(known) = self.0.borrow().get(&origin) {
            return known.clone();
        }
        let answer = fetch();
        self.0.borrow_mut().insert(origin, answer.clone());
        answer
    }
}
