//! The encodings DWARF reads: little-endian fixed widths, LEB128, strings ending in a zero.

/// Bytes of a section under construction.
#[derive(Default)]
pub(crate) struct Out(pub(crate) Vec<u8>);

impl Out {
    pub(crate) fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    pub(crate) fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn uleb(&mut self, mut v: u64) {
        loop {
            let byte = u8::try_from(v & 0x7f).expect("seven bits");
            v >>= 7;
            if v == 0 {
                return self.u8(byte);
            }
            self.u8(byte | 0x80);
        }
    }

    pub(crate) fn sleb(&mut self, mut v: i64) {
        loop {
            let byte = u8::try_from(v & 0x7f).expect("seven bits");
            v >>= 7;
            let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
            if done {
                return self.u8(byte);
            }
            self.u8(byte | 0x80);
        }
    }

    pub(crate) fn str(&mut self, s: &[u8]) {
        self.0.extend_from_slice(s);
        self.u8(0);
    }

    /// Where the next byte goes.
    pub(crate) fn here(&self) -> u64 {
        self.0.len() as u64
    }

    /// Writes a 32-bit length at `at`: the bytes after it up to here.
    pub(crate) fn patch_length(&mut self, at: u64) {
        let at = usize::try_from(at).expect("sections fit memory");
        let length = u32::try_from(self.0.len() - at - 4).expect("sections fit 4 GiB");
        self.0[at..at + 4].copy_from_slice(&length.to_le_bytes());
    }
}
