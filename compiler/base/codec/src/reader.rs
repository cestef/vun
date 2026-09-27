//! Reading a stream: its bytes, and the pools its handles are read from.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use vun_par::Map;

use crate::{Blob, Decode, Malformed, Pooled, Pools};

/// What the values of one stream are read against: its input, its pools as places in the input,
/// and the handles already read from them, so that each entry is recorded once however often it
/// is referenced. Threads share it without locks, so values can be read when first needed, in
/// any order, on any thread.
#[derive(Default)]
pub struct Context {
    input: Box<[u8]>,
    pools: HashMap<String, Vec<Blob>>,
    /// The handles read from each pool, by the handle's type.
    read: Map<TypeId, Arc<dyn Any + Send + Sync>>,
}

/// Each entry of one pool's handle, once read.
struct PoolReader<T: Pooled> {
    handles: Box<[OnceLock<T>]>,
}

impl Context {
    /// A stream of `input`, whose pools are the places `pools` give in it.
    #[must_use]
    pub fn within(input: Box<[u8]>, pools: Pools<Blob>) -> Self {
        let pools = pools.0.into_iter().map(|pool| (pool.name, pool.entries)).collect();
        Self { input, pools, read: Map::default() }
    }

    /// A stream with `pools`, as written.
    #[must_use]
    pub fn new(pools: Pools) -> Self {
        let mut input = Vec::new();
        let pools = pools.0.into_iter().map(|pool| {
            let entries = pool.entries.iter().map(|entry| {
                let at = input.len();
                input.extend_from_slice(entry);
                Blob { at, len: entry.len() }
            });
            crate::Pool { name: pool.name, entries: entries.collect() }
        });
        let pools = Pools(pools.collect());
        Self::within(input.into_boxed_slice(), pools)
    }

    /// The bytes of `blob`, read from this stream's input.
    #[must_use]
    pub fn bytes(&self, blob: Blob) -> &[u8] {
        blob.of(&self.input)
    }

    /// Reads a value from the front of `bytes`, as a header leading them, which holds no handle.
    ///
    /// # Errors
    /// Returns why the front of the bytes is not a value of this type.
    pub fn front<T: Decode>(&self, bytes: &[u8]) -> Result<T, Malformed> {
        T::decode(&mut Reader { bytes, at: 0, context: self })
    }

    /// Reads a value that is the whole of `bytes`, recording the entries its handles stand for in
    /// the process's tables.
    ///
    /// # Errors
    /// Returns why the bytes are not exactly one value of this type.
    pub fn read<T: Decode>(&self, bytes: &[u8]) -> Result<T, Malformed> {
        Reader { bytes, at: 0, context: self }.whole()
    }

    /// Reads a value that is the whole of `blob`, a place in this stream's input.
    ///
    /// # Errors
    /// As [`Context::read`].
    pub fn read_blob<T: Decode>(&self, blob: Blob) -> Result<T, Malformed> {
        Reader { bytes: self.bytes(blob), at: blob.at, context: self }.whole()
    }

    /// The entries of each pool, as bytes.
    pub(crate) fn entries(&self) -> HashMap<String, Vec<Vec<u8>>> {
        let entries = |blobs: &Vec<Blob>| blobs.iter().map(|&blob| self.bytes(blob).to_vec()).collect();
        self.pools.iter().map(|(name, blobs)| (name.clone(), entries(blobs))).collect()
    }

    /// The handles of type `T` read so far, with their numbers in the stream.
    pub(crate) fn numbers<T: Pooled>(&self) -> HashMap<T, usize, foldhash::fast::RandomState> {
        let Some(pool) = self.read.pin().get(&TypeId::of::<T>()).cloned() else { return HashMap::default() };
        let pool = pool.downcast::<PoolReader<T>>().expect("one pool per handle type");
        pool.handles
            .iter()
            .enumerate()
            .filter_map(|(number, handle)| Some((*handle.get()?, number)))
            .collect()
    }

    /// The handles read from the pool of `T`, made the first time.
    fn pool<T: Pooled>(&self) -> Arc<PoolReader<T>> {
        let read = self.read.pin();
        let pool = read.get_or_insert_with(TypeId::of::<T>(), || {
            let count = self.pools.get(T::POOL).map_or(0, Vec::len);
            Arc::new(PoolReader::<T> { handles: std::iter::repeat_with(OnceLock::new).take(count).collect() })
        });
        Arc::clone(pool).downcast().expect("one pool per handle type")
    }
}

/// The bytes of a value being read, consumed from the front.
pub struct Reader<'a> {
    bytes: &'a [u8],
    /// Where `bytes` starts in the input.
    at: usize,
    context: &'a Context,
}

impl<'a> Reader<'a> {
    /// Reads a value holding no handle from the whole of `bytes`.
    pub(crate) fn plain<T: Decode>(bytes: &'a [u8]) -> Result<T, Malformed> {
        Self::plain_at(bytes, 0)
    }

    /// As [`Reader::plain`], for `bytes` starting at `at` in the input.
    pub(crate) fn plain_at<T: Decode>(bytes: &'a [u8], at: usize) -> Result<T, Malformed> {
        Reader { bytes, at, context: &Context::default() }.whole()
    }

    fn whole<T: Decode>(mut self) -> Result<T, Malformed> {
        let value = T::decode(&mut self)?;
        if self.bytes.is_empty() { Ok(value) } else { Err(Malformed::Trailing) }
    }

    /// Where the next byte is in the input.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.at
    }

    /// Takes `n` bytes from the front.
    ///
    /// # Errors
    /// Returns [`Malformed::Truncated`] if fewer remain.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let (taken, rest) = self.bytes.split_at_checked(n).ok_or(Malformed::Truncated)?;
        self.bytes = rest;
        self.at += n;
        Ok(taken)
    }

    /// Takes one byte from the front.
    ///
    /// # Errors
    /// Returns [`Malformed::Truncated`] at the end of the input.
    pub fn byte(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }

    /// How many bytes remain.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len()
    }

    /// Reads a handle by its number, recording its entry in the reading process's table the
    /// first time the stream references it.
    ///
    /// # Errors
    /// Returns why the number or its entry is malformed.
    pub fn pooled<T: Pooled>(&mut self) -> Result<T, Malformed> {
        let number = usize::decode(self)?;
        let context = self.context;
        let pool = context.pool::<T>();
        let slot = pool.handles.get(number).ok_or(Malformed::Unpooled { pool: T::POOL, number })?;
        if let Some(&handle) = slot.get() {
            return Ok(handle);
        }
        let entry = context.pools[T::POOL][number];
        // Reading the entry may read other entries, so the slot is set after: two threads may
        // both record it, which gives equal handles, and the first stands. An entry reached
        // again while it is read refers to itself, which no written stream does.
        let _reading = Reading::enter(context, TypeId::of::<T>(), number)?;
        let mut r = Reader { bytes: context.bytes(entry), at: entry.at, context };
        let handle = T::read(&mut r)?;
        if r.remaining() != 0 {
            return Err(Malformed::Trailing);
        }
        Ok(*slot.get_or_init(|| handle))
    }
}

/// A pool entry this thread is reading, by its stream, handle type and number.
type Entry = (usize, TypeId, usize);

thread_local! {
    static READING: RefCell<HashSet<Entry>> = RefCell::new(HashSet::new());
}

/// The mark of an entry being read on this thread, removed when dropped.
struct Reading(Entry);

impl Reading {
    fn enter(context: &Context, ty: TypeId, number: usize) -> Result<Self, Malformed> {
        let entry = (std::ptr::from_ref(context).addr(), ty, number);
        let fresh = READING.with_borrow_mut(|reading| reading.insert(entry));
        fresh.then_some(Self(entry)).ok_or(Malformed::Cycle)
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        READING.with_borrow_mut(|reading| reading.remove(&self.0));
    }
}
