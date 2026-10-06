//! The components of one kind a registry holds, each with where it comes from.

use std::sync::Arc;

use vun_trace::tracing::debug;

use super::Provider;

/// What a component of a kind is called, which no two of a registry share.
pub trait Named {
    fn called(&self) -> &str;
}

/// A component and where it comes from.
pub struct Registered<T: ?Sized> {
    pub item: Arc<T>,
    pub from: Provider,
}

pub struct Roster<T: ?Sized> {
    items: Vec<Registered<T>>,
}

impl<T: ?Sized> Default for Roster<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T: ?Sized + Named> Roster<T> {
    /// Registers `item`, compiled into `vun`.
    pub fn link(&mut self, item: Box<T>) {
        self.add(item.into(), Provider::Linked);
    }

    /// Registers `item`, loaded from the library at `path`.
    pub fn load(&mut self, item: Box<T>, path: std::path::PathBuf) {
        self.add(item.into(), Provider::Loaded(path));
    }

    /// Registers `item` as `from` provides it, unless one of its name is: the first registered,
    /// linked in or loaded, is the one used.
    pub fn add(&mut self, item: Arc<T>, from: Provider) {
        let name = item.called();
        if self.get(name).is_some() {
            debug!(component = name, "a component of this name is registered already");
        } else {
            self.items.push(Registered { item, from });
        }
    }

    /// The component called `name`, and where it comes from.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Registered<T>> {
        self.items.iter().find(|r| r.item.called() == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|r| &*r.item)
    }

    pub fn registered(&self) -> impl Iterator<Item = &Registered<T>> {
        self.items.iter()
    }
}
