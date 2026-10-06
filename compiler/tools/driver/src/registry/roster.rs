//! The components of one kind a registry holds, each with where it comes from.

use std::sync::Arc;

use vun_trace::tracing::debug;

use super::Provider;

/// What a component of a kind is called, which no two of a registry share.
pub(super) trait Named {
    fn called(&self) -> &str;
}

/// A component and where it comes from.
pub(super) struct Registered<T: ?Sized> {
    pub(super) item: Arc<T>,
    pub(super) from: Provider,
}

pub(super) struct Roster<T: ?Sized> {
    items: Vec<Registered<T>>,
}

impl<T: ?Sized> Default for Roster<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T: ?Sized + Named> Roster<T> {
    /// Registers `item` unless one of its name is: the first registered, linked in or loaded, is
    /// the one used.
    pub(super) fn add(&mut self, item: Arc<T>, from: Provider) {
        let name = item.called();
        if self.get(name).is_some() {
            debug!(component = name, "a component of this name is registered already");
        } else {
            self.items.push(Registered { item, from });
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<&Registered<T>> {
        self.items.iter().find(|r| r.item.called() == name)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|r| &*r.item)
    }

    pub(super) fn registered(&self) -> impl Iterator<Item = &Registered<T>> {
        self.items.iter()
    }
}
