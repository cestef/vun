//! Relocations with their addends: where the linker writes, what it writes there (a number each
//! machine gives its kinds, `arm::Reloc` and its siblings), from which symbol, plus how much.

use vun_bytes::Out;

use crate::headers::RelaEntry;
use crate::input::{Input, Malformed};
use crate::record::{SignedWord, Word};
use crate::{Class, Section};

/// A relocation: the offset in its section it writes at, the number of the symbol it names in
/// the linked symbol table, its kind's number, and its addend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rela {
    pub offset: u64,
    pub symbol: u32,
    pub kind: u32,
    pub addend: i64,
}

impl Class {
    /// The low bytes of a relocation's info its kind takes, and the bytes above them its symbol's
    /// number takes.
    const fn info_bytes(self) -> (usize, usize) {
        match self {
            Self::Elf32 => (1, 3),
            Self::Elf64 => (4, 4),
        }
    }

    /// A relocation's info: `symbol` above `kind`.
    ///
    /// # Panics
    /// Panics on a number past the bytes the class gives it.
    fn info(self, symbol: u32, kind: u32) -> u64 {
        let (low, high) = self.info_bytes();
        let mut info = [0; 8];
        for (bytes, value, at) in [(low, kind, 0), (high, symbol, low)] {
            let value = value.to_le_bytes();
            assert!(value[bytes..].iter().all(|&b| b == 0), "a relocation's number fits its bytes");
            info[at..at + bytes].copy_from_slice(&value[..bytes]);
        }
        u64::from_le_bytes(info)
    }

    /// The symbol's number and the kind a relocation's info holds.
    fn of_info(self, info: u64) -> (u32, u32) {
        let (low, high) = self.info_bytes();
        let info = info.to_le_bytes();
        let number = |bytes: &[u8]| {
            let mut value = [0; 4];
            value[..bytes.len()].copy_from_slice(bytes);
            u32::from_le_bytes(value)
        };
        (number(&info[low..low + high]), number(&info[..low]))
    }
}

impl Rela {
    /// The size of a relocation section's entry in `class`.
    #[must_use]
    pub fn size(class: Class) -> u64 {
        RelaEntry::size(class)
    }

    /// The bytes of a relocation section of `class` holding `relocations`.
    ///
    /// # Panics
    /// Panics on a kind or a symbol's number past the bytes the class gives it.
    #[must_use]
    pub fn table(class: Class, relocations: &[Self]) -> Vec<u8> {
        let mut out = Out::default();
        for r in relocations {
            let info = Word(class.info(r.symbol, r.kind));
            RelaEntry { offset: Word(r.offset), info, addend: SignedWord(r.addend) }.put(class, &mut out);
        }
        out.0
    }

    /// The relocations `section`, a relocation section of `class`, holds.
    ///
    /// # Errors
    /// Where an entry runs past the section's end.
    pub fn read(class: Class, section: &Section) -> Result<Vec<Self>, Malformed> {
        let mut input = Input::at(section.bytes(), 0);
        (0..section.bytes().len() as u64 / RelaEntry::size(class))
            .map(|_| {
                let entry = RelaEntry::take(class, &mut input)?;
                let (symbol, kind) = class.of_info(entry.info.0);
                Ok(Self { offset: entry.offset.0, symbol, kind, addend: entry.addend.0 })
            })
            .collect()
    }
}
