//! Writing an object in its format.

mod elf;
mod macho;

use crate::{Format, Object};

impl Object {
    /// The object file.
    ///
    /// # Panics
    /// Panics on a relocation of a kind the machine does not have in the format, or on an object
    /// past the format's counts.
    #[must_use]
    pub fn write(&self) -> Vec<u8> {
        match self.format {
            Format::Elf => self.elf(),
            Format::MachO => self.macho(),
        }
    }
}

/// A string table under construction, starting with the empty string.
#[derive(Default)]
struct Strings(Vec<u8>);

impl Strings {
    fn add(&mut self, s: &[u8]) -> u32 {
        if self.0.is_empty() {
            self.0.push(0);
        }
        let at = u32::try_from(self.0.len()).expect("string tables fit 4 GiB");
        self.0.extend_from_slice(s);
        self.0.push(0);
        at
    }
}
