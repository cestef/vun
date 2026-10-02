//! Reading a file: its headers, then its sections from their headers, then the tables the
//! headers point to.

use std::fmt;

use vun_bytes::{Copies, In, NAME_MAX, Overspent, Truncated};

use crate::{Image, Machine, Section, SectionFlags};

/// Why bytes are not a PE file this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Something ends past the file's end.
    Truncated(Truncated),
    /// The file does not start as a DOS program does, `MZ`.
    Dos,
    /// The header the DOS header points to is not `PE\0\0`.
    Signature,
    /// The optional header's magic names neither PE32 nor PE32+.
    Magic(u16),
    /// A table lies where no section is loaded.
    Rva(u64),
    /// An address, or the end of a section, is past the end of the address space.
    Address(u64),
    /// A name loaded at this offset has no end within the longest the readers copy.
    Name(u64),
    /// The file has more sections than the format allows.
    Sections(u16),
    /// The file shares bytes a well-formed one holds once each: sections over the same bytes of
    /// the file, or imports in the same slots.
    Overlap,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(why) => write!(f, "{why}"),
            Self::Dos => f.write_str("the file does not start as a DOS program does"),
            Self::Signature => f.write_str("the file has no PE signature where its DOS header points"),
            Self::Magic(magic) => write!(f, "{magic:#x} is no optional header magic"),
            Self::Rva(rva) => write!(f, "{rva:#x} is loaded in no section"),
            Self::Address(rva) => write!(f, "{rva:#x} is past the end of the address space"),
            Self::Name(rva) => write!(f, "the name at {rva:#x} has no end within {NAME_MAX} bytes"),
            Self::Sections(count) => write!(f, "{count} sections are more than {SECTIONS_MAX}"),
            Self::Overlap => f.write_str("the file shares bytes between sections or import slots"),
        }
    }
}

impl std::error::Error for Malformed {}

/// `MZ`, the DOS header's first word.
const DOS_MAGIC: u16 = 0x5a4d;
/// Where the DOS header says the PE header is.
const PE_OFFSET: u64 = 0x3c;
const SIGNATURE: [u8; 4] = *b"PE\0\0";
/// The optional header's magic for 32-bit and for 64-bit images.
const PE32: u16 = 0x010b;
const PE32_PLUS: u16 = 0x020b;
/// `IMAGE_FILE_DLL`, among the file header's characteristics.
const DLL: u16 = 0x2000;
/// Bytes of a section header.
const SECTION_HEADER: u64 = 40;
/// The most sections a file has: the loader refuses more.
const SECTIONS_MAX: u16 = 96;

/// The data directories of an image: the tables its optional header points to.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Directories {
    pub(crate) export: (u64, u64),
    pub(crate) import: (u64, u64),
}

impl Image {
    /// The image `bytes` hold.
    ///
    /// # Errors
    /// Where they are not a PE file, or something in them runs past their end.
    pub fn read(bytes: &[u8]) -> Result<Self, Malformed> {
        let mut input = In::at(bytes, 0);
        if input.u16()? != DOS_MAGIC {
            return Err(Malformed::Dos);
        }
        input.seek(PE_OFFSET);
        let header = u64::from(input.u32()?);
        input.seek(header);
        if input.array::<4>()? != SIGNATURE {
            return Err(Malformed::Signature);
        }
        let machine = Machine::of(input.u16()?);
        let count = input.u16()?;
        if count > SECTIONS_MAX {
            return Err(Malformed::Sections(count));
        }
        // The time stamp, and the symbol table, which a program has no use of.
        input.take(12)?;
        let optional_size = u64::from(input.u16()?);
        let library = input.u16()? & DLL != 0;
        let optional = input.position();
        let (wide, entry, base, directories) = optional_header(bytes, optional)?;
        input.seek(optional + optional_size);
        let mut image = Self {
            machine,
            library,
            wide,
            base,
            entry: 0,
            sections: Vec::new(),
            exports: Vec::new(),
            imports: Vec::new(),
        };
        image.entry = if entry == 0 { 0 } else { image.address(entry)? };
        let mut copies = Copies::of(bytes.len() as u64);
        for _ in 0..count {
            image.sections.push(section(bytes, &mut input, base, &mut copies)?);
        }
        image.exports = image.exports_at(directories.export)?;
        image.imports = image.imports_at(directories.import)?;
        Ok(image)
    }

    /// Where the offset `rva` of the image is loaded.
    pub(crate) fn address(&self, rva: u64) -> Result<u64, Malformed> {
        self.base.checked_add(rva).ok_or(Malformed::Address(rva))
    }

    /// The section holding the offset `rva` of the image, and how far into it that is.
    fn loaded(&self, rva: u64) -> Result<(&Section, u64), Malformed> {
        let address = self.address(rva)?;
        let section = self
            .sections
            .iter()
            .find(|s| address >= s.address && address - s.address < s.size)
            .ok_or(Malformed::Rva(rva))?;
        Ok((section, address - section.address))
    }

    /// The `n` bytes loaded at the offset `rva` of the image, which a section holds.
    pub(crate) fn at_rva(&self, rva: u64, n: u64) -> Result<&[u8], Malformed> {
        // A table of nothing is where nothing says: a library exporting by ordinal alone has no
        // names, and its table of them may be anywhere, or nowhere.
        if n == 0 {
            return Ok(&[]);
        }
        let (section, from) = self.loaded(rva)?;
        let bounds = usize::try_from(from)
            .ok()
            .zip(usize::try_from(from.checked_add(n).ok_or(Malformed::Rva(rva))?).ok());
        let (from, to) = bounds.ok_or(Malformed::Rva(rva))?;
        section.data.get(from..to).ok_or(Malformed::Rva(rva))
    }

    /// The zero-ended name loaded at the offset `rva`.
    pub(crate) fn string_at_rva(&self, rva: u64) -> Result<&[u8], Malformed> {
        let (section, from) = self.loaded(rva)?;
        In::name(&section.data, from).ok_or(Malformed::Name(rva))
    }
}

/// The optional header at `at`: whether the image is 64-bit, its entry's offset, where it is
/// loaded and the directories it names.
fn optional_header(bytes: &[u8], at: u64) -> Result<(bool, u64, u64, Directories), Malformed> {
    let mut input = In::at(bytes, at);
    let wide = match input.u16()? {
        PE32 => false,
        PE32_PLUS => true,
        magic => return Err(Malformed::Magic(magic)),
    };
    input.seek(at + 16);
    let entry = u64::from(input.u32()?);
    // After the base of code come the base of data (PE32 only) and the base.
    input.seek(at + if wide { 24 } else { 28 });
    let base = if wide { input.u64()? } else { u64::from(input.u32()?) };
    // The count of directories, then the directories, each an offset and a size.
    let directories_at = at + if wide { 108 } else { 92 };
    input.seek(directories_at);
    let count = input.u32()?;
    let mut directory = |k: u32| -> Result<(u64, u64), Malformed> {
        if k >= count {
            return Ok((0, 0));
        }
        input.seek(directories_at + 4 + 8 * u64::from(k));
        Ok((u64::from(input.u32()?), u64::from(input.u32()?)))
    };
    Ok((wide, entry, base, Directories { export: directory(0)?, import: directory(1)? }))
}

/// The section header `input` is at, and the section it describes.
fn section(bytes: &[u8], input: &mut In<'_>, base: u64, copies: &mut Copies) -> Result<Section, Malformed> {
    let header = input.position();
    let raw_name = input.array::<8>()?;
    let size = u64::from(input.u32()?);
    let rva = u64::from(input.u32()?);
    let raw_size = u64::from(input.u32()?);
    let raw = u64::from(input.u32()?);
    input.seek(header + 36);
    let flags = SectionFlags(input.u32()?);
    input.seek(header + SECTION_HEADER);
    let name = raw_name.iter().take_while(|&&b| b != 0).copied().collect();
    // What the file holds: at most the section's raw size, and no more than it is loaded as.
    let held = if size == 0 { raw_size } else { raw_size.min(size) };
    let data = In::at(bytes, raw).take(held)?;
    copies.spend(held)?;
    // Every address in the section, and its end, is one.
    let size = size.max(held);
    let address = base
        .checked_add(rva)
        .filter(|address| address.checked_add(size).is_some())
        .ok_or(Malformed::Address(rva))?;
    Ok(Section { name, address, size, data: data.to_vec(), flags })
}

impl From<Overspent> for Malformed {
    fn from(Overspent: Overspent) -> Self {
        Self::Overlap
    }
}
