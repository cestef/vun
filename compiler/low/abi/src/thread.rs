//! How a platform reaches the calling thread's storage of a datum (spec 07, section 2), which each
//! backend declares for its machine beside the registers of a convention.

/// The sequence reaching a thread's storage, the register it gives the address in and those it
/// overwrites besides. `I` is a general register.
#[derive(Clone, Copy, Debug)]
pub struct Thread<I: 'static> {
    pub access: Access,
    pub address: I,
    pub overwrites: &'static [I],
}

/// How code reaches a thread's storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Through its TLS descriptor (ELF), whose call gives the storage's offset from the thread
    /// pointer, and which a linker rewrites into the offset where it knows it, in an executable.
    Descriptor,
    /// Through its thread-local variable (Mach-O): a descriptor whose call gives the storage's
    /// address.
    Variable,
    /// By its offset from the thread pointer, a word the linker writes beside the code, which only
    /// an executable knows.
    Offset,
}
