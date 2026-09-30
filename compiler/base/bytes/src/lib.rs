//! Binary encodings the object writers share (DWARF sections, wasm modules, symbol tables):
//! little-endian fixed widths, LEB128, LEB128 padded to a fixed width, which a linker rewrites in
//! place, and tables of strings each ended by a zero byte.

#[cfg(test)]
mod tests;

/// Bytes under construction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Out(pub Vec<u8>);

/// The bytes a 32-bit LEB128 takes at most, which a padded one always takes.
pub const PADDED: usize = 5;

impl Out {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }

    /// `n` zero bytes, a field something later fills.
    pub fn zeros(&mut self, n: usize) {
        self.0.resize(self.0.len() + n, 0);
    }

    pub fn uleb(&mut self, mut v: u64) {
        loop {
            let byte = Self::low7(v);
            v >>= 7;
            if v == 0 {
                return self.u8(byte);
            }
            self.u8(byte | 0x80);
        }
    }

    pub fn sleb(&mut self, mut v: i64) {
        loop {
            #[expect(clippy::cast_sign_loss, reason = "the low seven bits of two's complement")]
            let byte = Self::low7(v as u64);
            v >>= 7;
            let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
            if done {
                return self.u8(byte);
            }
            self.u8(byte | 0x80);
        }
    }

    /// `v` as an unsigned LEB128 of [`PADDED`] bytes, continuation bits set on all but the last.
    pub fn uleb_padded(&mut self, v: u32) {
        self.padded(u64::from(v));
    }

    /// `v` as a signed LEB128 of [`PADDED`] bytes.
    pub fn sleb_padded(&mut self, v: i32) {
        #[expect(clippy::cast_sign_loss, reason = "two's complement bits, of which 35 are written")]
        self.padded(i64::from(v) as u64);
    }

    fn padded(&mut self, v: u64) {
        for i in 0..PADDED {
            let byte = Self::low7(v >> (7 * i));
            self.u8(if i + 1 < PADDED { byte | 0x80 } else { byte });
        }
    }

    fn low7(v: u64) -> u8 {
        u8::try_from(v & 0x7f).expect("seven bits")
    }

    /// `s`, then a zero byte.
    pub fn str(&mut self, s: &[u8]) {
        self.bytes(s);
        self.u8(0);
    }

    /// Where the next byte goes.
    #[must_use]
    pub fn here(&self) -> u64 {
        self.0.len() as u64
    }

    /// Writes a 32-bit length at `at`: the bytes after it up to here.
    ///
    /// # Panics
    /// Panics past 4 GiB.
    pub fn patch_length(&mut self, at: u64) {
        let at = usize::try_from(at).expect("sections fit memory");
        let length = u32::try_from(self.0.len() - at - 4).expect("sections fit 4 GiB");
        self.0[at..at + 4].copy_from_slice(&length.to_le_bytes());
    }

    /// Rewrites the padded unsigned LEB128 at `at` as `v`, as a linker relocates one.
    ///
    /// # Panics
    /// Panics if the bytes end before it does.
    pub fn rewrite_uleb_padded(&mut self, at: usize, v: u32) {
        let mut field = Self::default();
        field.uleb_padded(v);
        self.rewrite(at, &field.0);
    }

    /// Rewrites the padded signed LEB128 at `at` as `v`.
    ///
    /// # Panics
    /// Panics if the bytes end before it does.
    pub fn rewrite_sleb_padded(&mut self, at: usize, v: i32) {
        let mut field = Self::default();
        field.sleb_padded(v);
        self.rewrite(at, &field.0);
    }

    /// Rewrites the little-endian 32 bits at `at` as `v`.
    ///
    /// # Panics
    /// Panics if the bytes end before they do.
    pub fn rewrite_u32(&mut self, at: usize, v: u32) {
        self.rewrite(at, &v.to_le_bytes());
    }

    fn rewrite(&mut self, at: usize, field: &[u8]) {
        self.0[at..at + field.len()].copy_from_slice(field);
    }
}

/// A table of strings, each ended by a zero byte and named by where it starts, the first the empty
/// string: ELF's and Mach-O's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Strings(Vec<u8>);

impl Strings {
    /// Adds `s`, giving where it starts.
    ///
    /// # Panics
    /// Panics past 4 GiB.
    pub fn add(&mut self, s: &[u8]) -> u32 {
        if self.0.is_empty() {
            self.0.push(0);
        }
        let at = u32::try_from(self.0.len()).expect("string tables fit 4 GiB");
        self.0.extend_from_slice(s);
        self.0.push(0);
        at
    }

    /// The table's bytes, the empty string's at least.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        if self.0.is_empty() { vec![0] } else { self.0 }
    }
}
