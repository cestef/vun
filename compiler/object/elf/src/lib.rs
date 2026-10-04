//! ELF files as data (the System V ABI, chapter 4, and each machine's supplement), little-endian
//! of either class: the file's kind and machine, its sections with their names, addresses and
//! bytes, its segments by what each covers, and the entries of its symbol tables and
//! relocations. Each record the file lays out is declared once (`headers`), and [`File::write`]
//! and [`File::read`] go through the same declarations, so that what one writes the other reads
//! as it was.

#[macro_use]
mod numbered;
#[macro_use]
mod record;

pub mod aarch64;
pub mod arm;
mod class;
mod headers;
mod loads;
mod malformed;
mod read;
mod rela;
mod section;
mod segment;
mod symbol;
#[cfg(test)]
mod tests;
mod write;
pub mod x86_64;

pub use class::Class;
pub use loads::Load;
pub use malformed::Malformed;
pub use rela::Rela;
pub use section::{Contents, Section, SectionFlags, SectionKind};
pub use segment::{Covers, Segment, SegmentFlags, SegmentKind};
pub use symbol::{Binding, Symbol, SymbolKind};

/// An ELF file: its class, kind and machine, the machine's flags, where a program starts, its
/// sections in order, the null section left out (section `k` of the file is `sections[k - 1]`),
/// and its segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub class: Class,
    pub kind: Kind,
    pub machine: Machine,
    pub flags: u32,
    pub entry: u64,
    pub sections: Vec<Section>,
    pub segments: Vec<Segment>,
}

numbered! {
    /// What a file is (`ET_*`).
    pub enum Kind: u16 {
        Relocatable = 1,
        Executable = 2,
        Shared = 3,
    }
}

numbered! {
    /// The machine a file's code is for (`EM_*`).
    pub enum Machine: u16 {
        /// 32-bit Arm.
        Arm = 40,
        X86_64 = 62,
        Aarch64 = 183,
    }
}

/// The file's first bytes.
const MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
/// `EV_CURRENT`, and `ELFDATA2LSB`.
const CURRENT: u8 = 1;
const LITTLE: u8 = 1;

impl File {
    /// The section `name`, if the file has one.
    #[must_use]
    pub fn section(&self, name: &[u8]) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// The section numbered `k` in the file, the null one being 0.
    #[must_use]
    pub fn numbered(&self, k: u32) -> Option<&Section> {
        self.sections.get(usize::try_from(k).ok()?.checked_sub(1)?)
    }

    /// The number the file gives `sections[at]`.
    ///
    /// # Panics
    /// Panics past `u32::MAX` sections.
    #[must_use]
    pub fn number(at: usize) -> u32 {
        u32::try_from(at + 1).expect("sections fit 32 bits")
    }

    /// The bytes loaded at `address` on, to the end of the section loaded there.
    #[must_use]
    pub fn loaded(&self, address: u64) -> Option<&[u8]> {
        self.sections.iter().filter(|s| s.flags.contains(SectionFlags::ALLOC)).find_map(|s| {
            let at = usize::try_from(address.checked_sub(s.address)?).ok()?;
            s.bytes().get(at..).filter(|_| (at as u64) < s.size())
        })
    }
}
