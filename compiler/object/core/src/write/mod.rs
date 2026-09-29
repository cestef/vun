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
