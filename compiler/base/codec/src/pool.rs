//! Handles into a process's own tables, written by content once per stream.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use crate::{Decode, Encode, Malformed, Reader, Writer};

/// A handle whose meaning is an entry of a table of the process, such as a file or an origin: in
/// a stream, its first occurrence writes the entry into the pool of its type and every occurrence
/// is the entry's number; reading it records the entry in the reader's table, once per stream.
pub trait Pooled: Copy + Eq + Hash + 'static {
    /// Names the pool in the stream.
    const POOL: &'static str;
    /// The table the handle is into, one per [`Resources`].
    type Table: Default + Send + Sync + 'static;

    /// Writes the entry `self` stands for.
    fn write(self, table: &Self::Table, w: &mut Writer<'_>);

    /// Reads an entry and records it in `table`.
    ///
    /// # Errors
    /// Returns why the bytes are not an entry.
    fn read(table: &Self::Table, r: &mut Reader<'_>) -> Result<Self, Malformed>;
}

/// Implements [`Encode`] and [`Decode`] for a [`Pooled`] handle, as its number in its pool.
#[macro_export]
macro_rules! pooled {
    ($ty:ty) => {
        impl $crate::Encode for $ty {
            fn encode(&self, w: &mut $crate::Writer<'_>) {
                w.pooled(*self);
            }
        }

        impl $crate::Decode for $ty {
            fn decode(r: &mut $crate::Reader<'_>) -> Result<Self, $crate::Malformed> {
                r.pooled()
            }
        }
    };
}

/// The tables a stream's pooled handles are into: shared per process, made on first use.
pub trait Resources {
    fn resource(&self, ty: TypeId, make: fn() -> Arc<dyn Any + Send + Sync>) -> Arc<dyn Any + Send + Sync>;
}

impl dyn Resources + '_ {
    /// The table of type `T`.
    ///
    /// # Panics
    /// If the resources hold another type under the type of `T`.
    #[must_use]
    pub fn get<T: Default + Send + Sync + 'static>(&self) -> Arc<T> {
        self.resource(TypeId::of::<T>(), || Arc::new(T::default())).downcast().expect("one resource per type")
    }
}

/// Resources on their own, one table per type: what streams read and write outside a database.
#[derive(Default)]
pub struct Tables(Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>);

impl Resources for Tables {
    fn resource(&self, ty: TypeId, make: fn() -> Arc<dyn Any + Send + Sync>) -> Arc<dyn Any + Send + Sync> {
        Arc::clone(self.0.lock().expect("tables").entry(ty).or_insert_with(make))
    }
}

/// The pools of a stream: each pool's entries, in the order they were first written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Pools(pub Vec<Pool>);

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Pool {
    pub name: String,
    pub entries: Vec<Vec<u8>>,
}
