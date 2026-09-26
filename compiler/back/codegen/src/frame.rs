//! The frame: everything below the frame pointer, laid out as it is needed, its size known when
//! the function ends.

/// The frame of the function compiled.
#[derive(Default)]
pub struct Frame {
    /// Bytes below the frame pointer taken so far.
    depth: u64,
    /// The most bytes a call's stack arguments take, at the bottom.
    outgoing: u64,
}

impl Frame {
    /// Reserves `size` bytes at `align`: their offset from the frame pointer.
    ///
    /// # Panics
    /// Panics past 2 GiB of frame, which offsets cannot reach.
    pub fn reserve(&mut self, size: u64, align: u64) -> i32 {
        self.depth = (self.depth + size).next_multiple_of(align.max(1));
        -i32::try_from(self.depth).expect("frames fit 2 GiB")
    }

    /// Makes room for `bytes` of a call's stack arguments.
    pub fn outgoing(&mut self, bytes: u64) {
        self.outgoing = self.outgoing.max(bytes);
    }

    /// The frame's size, aligned to `align`.
    #[must_use]
    pub fn size(&self, align: u64) -> u64 {
        (self.depth + self.outgoing).next_multiple_of(align)
    }
}
