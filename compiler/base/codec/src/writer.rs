//! Writing a stream: its bytes, and the pools its handles are written into.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{Encode, Pool, Pooled, Pools, Resources};

/// Where values are encoded: the bytes of the value being written, and the pools shared by every
/// value of the stream.
#[derive(Default)]
pub struct Writer<'r> {
    bytes: Vec<u8>,
    /// The pool of each handle type, in the order first written.
    pools: Vec<Box<dyn AnyPool>>,
    by_type: HashMap<TypeId, usize>,
    resources: Option<&'r dyn Resources>,
}

/// The entries written so far of one pool.
struct PoolWriter<T: Pooled> {
    table: Arc<T::Table>,
    numbers: HashMap<T, usize>,
    entries: Vec<Vec<u8>>,
}

trait AnyPool {
    fn as_any(&mut self) -> &mut dyn Any;
    fn into_pool(self: Box<Self>) -> Pool;
}

impl<T: Pooled> AnyPool for PoolWriter<T> {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn into_pool(self: Box<Self>) -> Pool {
        Pool { name: T::POOL.to_owned(), entries: self.entries }
    }
}

impl<'r> Writer<'r> {
    /// A stream holding no pooled handle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A stream whose pooled handles are into the tables of `resources`.
    #[must_use]
    pub fn pooling(resources: &'r dyn Resources) -> Self {
        Self { resources: Some(resources), ..Self::default() }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn byte(&mut self, byte: u8) {
        self.bytes.push(byte);
    }

    /// The bytes of `value` alone, its handles written into this stream's pools.
    pub fn encode<T: Encode + ?Sized>(&mut self, value: &T) -> Vec<u8> {
        self.encode_with(|w| value.encode(w))
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// The pools written, in the order they were first written to.
    #[must_use]
    pub fn into_pools(self) -> Pools {
        Pools(self.pools.into_iter().map(AnyPool::into_pool).collect())
    }

    /// Writes `handle` as its number in its pool, first writing its entry if it has none.
    ///
    /// # Panics
    /// If the stream has no resources to read the entry from.
    pub fn pooled<T: Pooled>(&mut self, handle: T) {
        let pool = self.pool::<T>();
        let number = if let Some(&number) = pool.numbers.get(&handle) {
            number
        } else {
            let table = Arc::clone(&pool.table);
            let entry = self.encode_with(|w| handle.write(&table, w));
            let pool = self.pool::<T>();
            pool.entries.push(entry);
            pool.numbers.insert(handle, pool.entries.len() - 1);
            pool.entries.len() - 1
        };
        number.encode(self);
    }

    fn encode_with(&mut self, write: impl FnOnce(&mut Self)) -> Vec<u8> {
        let outer = std::mem::take(&mut self.bytes);
        write(self);
        std::mem::replace(&mut self.bytes, outer)
    }

    fn pool<T: Pooled>(&mut self) -> &mut PoolWriter<T> {
        let index = *self.by_type.entry(TypeId::of::<T>()).or_insert_with(|| {
            let resources = self.resources.expect("a stream with pooled handles has resources");
            let table = resources.get::<T::Table>();
            self.pools.push(Box::new(PoolWriter::<T> {
                table,
                numbers: HashMap::new(),
                entries: Vec::new(),
            }));
            self.pools.len() - 1
        });
        self.pools[index].as_any().downcast_mut().expect("one pool per handle type")
    }
}
