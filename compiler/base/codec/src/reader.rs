//! Reading a stream: its bytes, and the pools its handles are read from.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use vun_par::Map;

use crate::{Blob, Decode, Malformed, Pooled, Pools};

/// The bytes of a stream's input, wherever they are held: in memory, or mapped from a file.
pub type Owned = Box<dyn AsRef<[u8]> + Send + Sync>;

/// What the values of one stream are read against: its input, its pools as places in the input,
/// and the handles already read from them, so that each entry is recorded once however often it
/// is referenced. Threads share it without locks, so values can be read when first needed, in
/// any order, on any thread.
///
/// A stream can be layered over another: its input follows the other's, so that a place names
/// the input of the layer holding it, and each of its pools numbers its entries after the other's
/// entries of that pool, so that its values name the other's handles as the other's values do.
pub struct Context {
    input: Owned,
    /// Where `input` starts in the stream: the end of the stream it is layered over.
    offset: usize,
    base: Option<Arc<Context>>,
    /// The entries of this layer's pools, as places in the stream.
    pools: HashMap<String, Vec<Blob>>,
    /// The handles read from each pool of this layer, by the handle's type.
    read: Map<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Default for Context {
    fn default() -> Self {
        Self::within(Box::new(Box::<[u8]>::default()), Pools::default())
    }
}

/// Each entry of one pool's handle, once read.
struct PoolReader<T: Pooled> {
    /// The number of its first entry: the entries of the layers below come before.
    first: usize,
    handles: Box<[OnceLock<T>]>,
}

impl Context {
    /// A stream of `input`, whose pools are the places `pools` give in it.
    #[must_use]
    pub fn within(input: Owned, pools: Pools<Blob>) -> Self {
        let pools = pools.0.into_iter().map(|pool| (pool.name, pool.entries)).collect();
        Self { input, offset: 0, base: None, pools, read: Map::default() }
    }

    /// A stream of `input` layered over `base`, whose pools are the places `pools` give in
    /// `input`.
    #[must_use]
    pub fn over(base: Arc<Context>, input: Owned, pools: Pools<Blob>) -> Self {
        let offset = base.end();
        let mut stream = Self { offset, base: Some(base), ..Self::within(input, Pools::default()) };
        let pools = pools
            .0
            .into_iter()
            .map(|pool| (pool.name, pool.entries.into_iter().map(|blob| stream.place(blob)).collect()));
        stream.pools = pools.collect();
        stream
    }

    /// The stream this one is layered over.
    #[must_use]
    pub fn base(&self) -> Option<&Context> {
        self.base.as_deref()
    }

    /// The place in the stream of `blob`, a place in this layer's input.
    #[must_use]
    pub fn place(&self, blob: Blob) -> Blob {
        Blob { at: self.offset + blob.at, len: blob.len }
    }

    /// Where the stream ends: where a stream layered over it starts.
    fn end(&self) -> usize {
        self.offset + self.input.as_ref().as_ref().len()
    }

    /// A stream with `pools`, as written.
    #[must_use]
    pub fn new(pools: Pools) -> Self {
        let (input, pools) = pools.placed();
        Self::within(Box::new(input), pools)
    }

    /// The bytes of `blob`, read from this stream's input.
    #[must_use]
    pub fn bytes(&self, blob: Blob) -> &[u8] {
        match &self.base {
            Some(base) if blob.at < self.offset => base.bytes(blob),
            _ => Blob { at: blob.at - self.offset, len: blob.len }.of(self.input.as_ref().as_ref()),
        }
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

    /// Reads a value from the front of `blob`, a place in this stream's input, as the places it
    /// holds being places in the stream; what follows it is not read.
    ///
    /// # Errors
    /// Returns why the front of the bytes is not a value of this type.
    pub fn front_blob<T: Decode>(&self, blob: Blob) -> Result<T, Malformed> {
        T::decode(&mut Reader { bytes: self.bytes(blob), at: blob.at, context: self })
    }

    /// The entries of each pool of this layer, as bytes.
    pub(crate) fn entries(&self) -> HashMap<String, Vec<Vec<u8>>> {
        let entries = |blobs: &Vec<Blob>| blobs.iter().map(|&blob| self.bytes(blob).to_vec()).collect();
        self.pools.iter().map(|(name, blobs)| (name.clone(), entries(blobs))).collect()
    }

    /// How many entries the pool `name` holds, in every layer.
    pub(crate) fn count(&self, name: &str) -> usize {
        self.first(name) + self.pools.get(name).map_or(0, Vec::len)
    }

    /// The number of the first entry of this layer's pool `name`.
    fn first(&self, name: &str) -> usize {
        self.base.as_ref().map_or(0, |base| base.count(name))
    }

    /// The handles of type `T` read so far, with their numbers in the stream.
    pub(crate) fn numbers<T: Pooled>(&self) -> HashMap<T, usize, foldhash::fast::RandomState> {
        let mut numbers = self.base.as_ref().map(|base| base.numbers()).unwrap_or_default();
        let Some(pool) = self.read.pin().get(&TypeId::of::<T>()).cloned() else { return numbers };
        let pool = pool.downcast::<PoolReader<T>>().expect("one pool per handle type");
        let read = pool.handles.iter().enumerate();
        numbers.extend(read.filter_map(|(number, handle)| Some((*handle.get()?, pool.first + number))));
        numbers
    }

    /// The handle numbered `number` in the pool of `T`, recording its entry in the reading
    /// process's table the first time the stream references it.
    fn handle<T: Pooled>(&self, number: usize) -> Result<T, Malformed> {
        let pool = self.pool::<T>();
        if let Some(base) = &self.base
            && number < pool.first
        {
            return base.handle(number);
        }
        let own = number - pool.first;
        let slot = pool.handles.get(own).ok_or(Malformed::Unpooled { pool: T::POOL, number })?;
        if let Some(&handle) = slot.get() {
            return Ok(handle);
        }
        let entry = self.pools[T::POOL][own];
        // Reading the entry may read other entries, so the slot is set after: two threads may
        // both record it, which gives equal handles, and the first stands. An entry reached
        // again while it is read refers to itself, which no written stream does.
        let _reading = Reading::enter(self, TypeId::of::<T>(), number)?;
        let mut r = Reader { bytes: self.bytes(entry), at: entry.at, context: self };
        let handle = T::read(&mut r)?;
        if r.remaining() != 0 {
            return Err(Malformed::Trailing);
        }
        Ok(*slot.get_or_init(|| handle))
    }

    /// The handles read from this layer's pool of `T`, made the first time.
    fn pool<T: Pooled>(&self) -> Arc<PoolReader<T>> {
        let read = self.read.pin();
        let pool = read.get_or_insert_with(TypeId::of::<T>(), || {
            let count = self.pools.get(T::POOL).map_or(0, Vec::len);
            let handles = std::iter::repeat_with(OnceLock::new).take(count).collect();
            Arc::new(PoolReader::<T> { first: self.first(T::POOL), handles })
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

    /// As [`Reader::plain_at`], whatever follows the value unread.
    pub(crate) fn plain_front<T: Decode>(bytes: &'a [u8], at: usize) -> Result<T, Malformed> {
        T::decode(&mut Reader { bytes, at, context: &Context::default() })
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
        self.context.handle(number)
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
