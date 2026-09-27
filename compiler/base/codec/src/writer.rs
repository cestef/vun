//! Writing a stream: its bytes, and the pools its handles are written into.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::{Context, Encode, Pool, Pooled, Pools};

/// Where values are encoded: the bytes of the value being written, and the pools shared by every
/// value of the stream.
#[derive(Default)]
pub struct Writer<'r> {
    bytes: Vec<u8>,
    /// The pool of each handle type, in the order first written.
    pools: Vec<Box<dyn AnyPool>>,
    by_type: HashMap<TypeId, usize>,
    /// The stream this one extends, whose pools it starts from.
    extends: Option<&'r Context>,
    /// The pools of that stream not written to yet, by name.
    carried: HashMap<String, Vec<Vec<u8>>>,
}

/// The entries written so far of one pool.
struct PoolWriter<T: Pooled> {
    numbers: HashMap<T, usize, foldhash::fast::RandomState>,
    entries: Vec<Vec<u8>>,
}

trait AnyPool {
    fn as_any(&mut self) -> &mut dyn Any;
    fn name(&self) -> &'static str;
    fn into_pool(self: Box<Self>) -> Pool;
}

impl<T: Pooled> AnyPool for PoolWriter<T> {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        T::POOL
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

    /// A stream extending `stream`: its pools start as that stream's, so that the bytes of a value
    /// of that stream are the same value in this one, and are written as they are.
    #[must_use]
    pub fn extending(stream: &'r Context) -> Self {
        Self { extends: Some(stream), carried: stream.entries(), ..Self::default() }
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

    /// The pools written, in the order they were first written to, then those carried from the
    /// stream extended and not written to, by name.
    #[must_use]
    pub fn into_pools(self) -> Pools {
        let mut carried: Vec<Pool> =
            self.carried.into_iter().map(|(name, entries)| Pool { name, entries }).collect();
        carried.sort_by(|a, b| a.name.cmp(&b.name));
        Pools(self.pools.into_iter().map(AnyPool::into_pool).chain(carried).collect())
    }

    /// Writes `handle` as its number in its pool, first writing its entry if it has none.
    pub fn pooled<T: Pooled>(&mut self, handle: T) {
        let number = if let Some(&number) = self.pool::<T>().numbers.get(&handle) {
            number
        } else {
            let entry = self.encode_with(|w| handle.write(w));
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
            // Pools are read back by name: two handle types sharing one would read each other's.
            assert!(
                self.pools.iter().all(|p| p.name() != T::POOL),
                "two handle types share the pool name `{}`",
                T::POOL
            );
            let entries = self.carried.remove(T::POOL).unwrap_or_default();
            let numbers = self.extends.map(Context::numbers).unwrap_or_default();
            self.pools.push(Box::new(PoolWriter::<T> { numbers, entries }));
            self.pools.len() - 1
        });
        self.pools[index].as_any().downcast_mut().expect("one pool per handle type")
    }
}
