//! Reading a stream: its bytes, and the pools its handles are read from.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{Decode, Malformed, Pooled, Pools, Resources};

/// What the values of one stream are read against: its pools, and the handles already read from
/// them, so that each entry is recorded once however often it is referenced.
#[derive(Default)]
pub struct Context<'a> {
    entries: HashMap<&'a str, &'a [Vec<u8>]>,
    read: RefCell<HashMap<TypeId, Box<dyn Any>>>,
    resources: Option<&'a dyn Resources>,
}

/// The handles read so far from one pool, by number.
struct PoolReader<T: Pooled> {
    table: Arc<T::Table>,
    handles: Vec<Option<T>>,
}

impl<'a> Context<'a> {
    /// A stream with `pools`, whose entries are recorded in the tables of `resources`.
    #[must_use]
    pub fn new(pools: &'a Pools, resources: &'a dyn Resources) -> Self {
        let entries = pools.0.iter().map(|pool| (pool.name.as_str(), pool.entries.as_slice())).collect();
        Self { entries, read: RefCell::default(), resources: Some(resources) }
    }

    /// Reads a value from the front of `bytes`, as a header leading them.
    ///
    /// # Errors
    /// Returns why the front of the bytes is not a value of this type.
    pub fn front<T: Decode>(&self, bytes: &[u8]) -> Result<T, Malformed> {
        T::decode(&mut Reader { bytes, context: self })
    }

    /// Reads a value that is the whole of `bytes`.
    ///
    /// # Errors
    /// Returns why the bytes are not exactly one value of this type.
    pub fn read<T: Decode>(&self, bytes: &[u8]) -> Result<T, Malformed> {
        let mut r = Reader { bytes, context: self };
        let value = T::decode(&mut r)?;
        if r.bytes.is_empty() { Ok(value) } else { Err(Malformed::Trailing) }
    }
}

/// The bytes of a value being read, consumed from the front.
pub struct Reader<'a> {
    bytes: &'a [u8],
    context: &'a Context<'a>,
}

impl<'a> Reader<'a> {
    /// Takes `n` bytes from the front.
    ///
    /// # Errors
    /// Returns [`Malformed::Truncated`] if fewer remain.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let (taken, rest) = self.bytes.split_at_checked(n).ok_or(Malformed::Truncated)?;
        self.bytes = rest;
        Ok(taken)
    }

    /// Takes one byte from the front.
    ///
    /// # Errors
    /// Returns [`Malformed::Truncated`] at the end of the input.
    pub fn byte(&mut self) -> Result<u8, Malformed> {
        let (&byte, rest) = self.bytes.split_first().ok_or(Malformed::Truncated)?;
        self.bytes = rest;
        Ok(byte)
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
    ///
    /// # Panics
    /// If the stream has pools but no resources to record their entries in.
    pub fn pooled<T: Pooled>(&mut self) -> Result<T, Malformed> {
        let number = usize::decode(self)?;
        let context = self.context;
        let table = {
            let mut read = context.read.borrow_mut();
            let pool = read
                .entry(TypeId::of::<T>())
                .or_insert_with(|| {
                    let resources = context.resources.expect("a stream with pools has resources");
                    Box::new(PoolReader::<T> { table: resources.get(), handles: Vec::new() })
                })
                .downcast_mut::<PoolReader<T>>()
                .expect("one pool per handle type");
            if let Some(&Some(handle)) = pool.handles.get(number) {
                return Ok(handle);
            }
            Arc::clone(&pool.table)
        };
        let unpooled = Malformed::Unpooled { pool: T::POOL, number };
        let entry = context.entries.get(T::POOL).and_then(|entries| entries.get(number)).ok_or(unpooled)?;
        // Reading the entry may read other handles, so the pools are not borrowed meanwhile.
        let mut r = Reader { bytes: entry, context };
        let handle = T::read(&table, &mut r)?;
        if r.remaining() != 0 {
            return Err(Malformed::Trailing);
        }
        let mut read = context.read.borrow_mut();
        let pool = read.get_mut(&TypeId::of::<T>()).and_then(|p| p.downcast_mut::<PoolReader<T>>());
        let handles = &mut pool.expect("made above").handles;
        if handles.len() <= number {
            handles.resize(number + 1, None);
        }
        handles[number] = Some(handle);
        Ok(handle)
    }
}
