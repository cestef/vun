//! What crosses the boundary of a component: plain C types, no Rust type, allocation or thread.
//!
//! A buffer is owned by the side that allocated it and is released by that side's `free`, so
//! that each allocator frees only its own. A call fills `out` with its reply, which the caller
//! reads and then frees; `Status` says what the reply is.

use std::ffi::c_void;

/// The version of this layout: the first thing a loader reads of a library, before it trusts
/// any other field.
pub const ABI: u32 = 2;

/// The symbol every component exports: a function returning its [`Descriptor`].
pub const ENTRY: &[u8] = b"vun_component\0";

/// A kind of component, named by the crate that declares its interface (`Interface::KIND`):
/// the set is open, and nothing here lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Kind(pub &'static str);

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Bytes borrowed for the length of a call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Slice {
    ptr: *const u8,
    len: usize,
}

impl Slice {
    #[must_use]
    pub const fn of(bytes: &[u8]) -> Self {
        Self { ptr: bytes.as_ptr(), len: bytes.len() }
    }

    /// The bytes.
    ///
    /// # Safety
    /// `self` is from [`Slice::of`] on bytes alive for `'a`.
    #[must_use]
    pub unsafe fn bytes<'a>(self) -> &'a [u8] {
        // SAFETY: by the caller: a pointer and length of a live slice.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

/// Bytes a call hands over, freed by the side that allocated them.
#[repr(C)]
pub struct Owned {
    ptr: *mut u8,
    len: usize,
    cap: usize,
}

impl Owned {
    pub const EMPTY: Self = Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 };

    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        let mut bytes = std::mem::ManuallyDrop::new(bytes);
        Self { ptr: bytes.as_mut_ptr(), len: bytes.len(), cap: bytes.capacity() }
    }

    /// The bytes, as the vector they were.
    ///
    /// # Safety
    /// `self` is from [`Owned::new`] in this module's allocator, and not yet released.
    #[must_use]
    pub unsafe fn into_vec(self) -> Vec<u8> {
        if self.ptr.is_null() {
            return Vec::new();
        }
        // SAFETY: by the caller: the parts of a vector of this allocator, given up by `new`.
        unsafe { Vec::from_raw_parts(self.ptr, self.len, self.cap) }
    }

    /// The bytes copied out and released by `free`, the side's that allocated them.
    ///
    /// # Safety
    /// `self` is from [`Owned::new`] and `free` is its allocator's.
    pub unsafe fn copied(self, free: Free) -> Vec<u8> {
        let bytes = if self.ptr.is_null() {
            Vec::new()
        } else {
            // SAFETY: by the caller: a pointer and length of live bytes, until freed.
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }.to_vec()
        };
        // SAFETY: by the caller; the bytes are not read again.
        unsafe { free(self) };
        bytes
    }

    /// The `free` of a side: drops what it allocated.
    ///
    /// # Safety
    /// As [`Owned::into_vec`].
    pub unsafe extern "C" fn release(self) {
        // SAFETY: by the caller.
        drop(unsafe { self.into_vec() });
    }
}

/// How a call ended.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// `out` holds the reply.
    Done = 0,
    /// The callee panicked: `out` holds its message.
    Panicked = 1,
    /// The callee could not read the request: `out` holds why.
    Malformed = 2,
}

/// Calls of a component: its entry, and the `free` of what it allocated.
pub type Call = unsafe extern "C" fn(*const Host, Slice, *mut Owned) -> Status;
pub type Free = unsafe extern "C" fn(Owned);

/// What a library says of itself.
#[repr(C)]
pub struct Descriptor {
    /// [`ABI`]: first, so that it is read before any other field.
    pub abi: u32,
    /// Its [`Kind`], as text.
    pub kind: Slice,
    /// The release of the toolchain it was built in, as text.
    pub release: Slice,
    pub call: Call,
    pub free: Free,
}

// SAFETY: a descriptor is immutable static data: numbers, a static string and functions.
unsafe impl Sync for Descriptor {}

/// What a library tells the host of an activity it times: it began, or the innermost one ended.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    Begin = 0,
    End = 1,
}

/// How the host times what a component does: a beat, the number of the activity's kind and its
/// name, a string of the library, which stays mapped.
pub type Relay = unsafe extern "C" fn(Beat, u32, Slice);

/// What the host lends a component for one call: how it asks the host, answered in the host's
/// own terms.
#[repr(C)]
pub struct Host {
    pub(crate) context: *const c_void,
    pub(crate) ask: unsafe extern "C" fn(*const c_void, Slice, *mut Owned) -> Status,
    pub(crate) free: Free,
    /// Where the component's timings go, when the host is timing.
    pub(crate) relay: Option<Relay>,
}
