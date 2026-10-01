//! Reading a file: its headers, then its sections from their headers, then the tables the
//! headers point to.

use std::fmt;

use vun_bytes::{In, Truncated};

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
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(why) => write!(f, "{why}"),
            Self::Dos => f.write_str("the file does not start as a DOS program does"),
            Self::Signature => f.write_str("the file has no PE signature where its DOS header points"),
            Self::Magic(magic) => write!(f, "{magic:#x} is no optional header magic"),
            Self::Rva(rva) => write!(f, "{rva:#x} is loaded in no section"),
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
/// Bytes of a section header, and of the file header after the signature.
const SECTION_HEADER: u64 = 40;

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
            entry: if entry == 0 { 0 } else { base + entry },
            sections: Vec::new(),
            exports: Vec::new(),
            imports: Vec::new(),
        };
        for _ in 0..count {
            image.sections.push(section(bytes, &mut input, base)?);
        }
        image.exports = image.exports_at(directories.export)?;
        image.imports = image.imports_at(directories.import)?;
        Ok(image)
    }

    /// The `n` bytes loaded at the offset `rva` of the image, which a section holds.
    pub(crate) fn at_rva(&self, rva: u64, n: u64) -> Result<&[u8], Malformed> {
        let address = self.base + rva;
        let section = self
            .sections
            .iter()
            .find(|s| (s.address..s.address + s.size).contains(&address))
            .ok_or(Malformed::Rva(rva))?;
        let from = usize::try_from(address - section.address).map_err(|_| Malformed::Rva(rva))?;
        let to = usize::try_from(address - section.address + n).map_err(|_| Malformed::Rva(rva))?;
        section.data.get(from..to).ok_or(Malformed::Rva(rva))
    }

    /// The zero-ended string loaded at the offset `rva`.
    pub(crate) fn string_at_rva(&self, rva: u64) -> Result<&[u8], Malformed> {
        let address = self.base + rva;
        let section = self
            .sections
            .iter()
            .find(|s| (s.address..s.address + s.size).contains(&address))
            .ok_or(Malformed::Rva(rva))?;
        Ok(In::string(&section.data, address - section.address)?)
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
fn section(bytes: &[u8], input: &mut In<'_>, base: u64) -> Result<Section, Malformed> {
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
    let data = In::at(bytes, raw).take(held)?.to_vec();
    Ok(Section { name, address: base + rva, size: size.max(held), data, flags })
}
