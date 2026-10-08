//! The frame: everything below the frame pointer, laid out as it is needed, its size known when
//! the function ends.

use vun_backend::Missing;

/// The frame of the function compiled.
#[derive(Default)]
pub struct Frame {
    /// Bytes below the frame pointer taken so far.
    depth: u64,
    /// The most bytes a call's stack arguments take, at the bottom.
    outgoing: u64,
}

impl Frame {
    /// The most bytes of frame: its accesses are displacements of 32 bits, signed, and what stays
    /// below the limit leaves room for the words a function reserves besides.
    pub const LIMIT: u64 = 1 << 30;

    /// Whether `bytes` fit a frame.
    ///
    /// # Errors
    /// `Missing::Frame` where they do not.
    pub const fn fit(bytes: u64) -> Result<(), Missing> {
        if bytes > Self::LIMIT { Err(Missing::Frame(bytes)) } else { Ok(()) }
    }

    /// Reserves `size` bytes at `align`: their offset from the frame pointer.
    ///
    /// # Panics
    /// Panics past 2 GiB of frame, which offsets cannot reach.
    pub fn reserve(&mut self, size: u64, align: u64) -> i32 {
        self.depth = (self.depth + size).next_multiple_of(align.max(1));
        -i32::try_from(self.depth).expect("frames fit 2 GiB")
    }

    /// Makes room for `bytes` of a call's stack arguments.
    ///
    /// # Errors
    /// `Missing::Frame` where they do not fit.
    pub fn outgoing(&mut self, bytes: u64) -> Result<(), Missing> {
        Self::fit(bytes)?;
        self.outgoing = self.outgoing.max(bytes);
        Ok(())
    }

    /// Bytes below the frame pointer taken so far.
    #[must_use]
    pub const fn depth(&self) -> u64 {
        self.depth
    }

    /// The frame's size, aligned to `align`.
    #[must_use]
    pub fn size(&self, align: u64) -> u64 {
        (self.depth + self.outgoing).next_multiple_of(align)
    }
}
