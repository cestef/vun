//! The code of a file, whatever its format: the sections that hold instructions, and the symbols
//! that name places in them or in the data they refer to. A format is read into this once, and
//! the listing reads only it.

use std::fmt;

use crate::Isa;

/// A section of code, as loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub name: Vec<u8>,
    /// Where its first byte is: a virtual address, or, in a file not linked, an offset.
    pub address: u64,
    pub bytes: Vec<u8>,
}

impl Part {
    /// The code `bytes` loaded at `address`.
    ///
    /// # Errors
    /// Where it ends past the end of the address space, so that no address in it overflows.
    pub fn loaded(name: &[u8], address: u64, bytes: Vec<u8>) -> Result<Self, Error> {
        if address.checked_add(bytes.len() as u64).is_none() {
            let name = String::from_utf8_lossy(name);
            return Err(Error::Malformed(format!(
                "the section {name} reaches past the end of the address space"
            )));
        }
        Ok(Self { name: name.to_vec(), address, bytes })
    }
}

/// A name for a place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: Vec<u8>,
    pub address: u64,
    /// How many bytes it names, 0 where it names a place and not a run of them.
    pub size: u64,
    /// The code it is in, by its place in [`Code::parts`], none for a place in data.
    pub part: Option<usize>,
}

/// The code of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub isa: Isa,
    pub parts: Vec<Part>,
    pub symbols: Vec<Symbol>,
    /// Whether addresses are those of a loaded program, which a branch's target names a symbol
    /// by, and not offsets in a section.
    pub linked: bool,
}

/// Why a file cannot be listed.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The file is neither ELF nor PE.
    Format,
    /// The file is for a machine whose instructions are not read here.
    Machine(String),
    /// The file is malformed.
    Malformed(String),
    /// The file has no code labelled by a symbol of this name.
    Missing(Vec<u8>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format => f.write_str("the file is neither an ELF nor a PE file"),
            Self::Machine(machine) => write!(f, "no instruction set is read for the machine {machine}"),
            Self::Malformed(why) => write!(f, "the file is malformed: {why}"),
            Self::Missing(name) => write!(f, "no code is labelled `{}`", String::from_utf8_lossy(name)),
        }
    }
}

impl std::error::Error for Error {}

/// The first bytes of an ELF file, and of a DOS program, which a PE file starts as.
const ELF: &[u8] = b"\x7fELF";
const DOS: &[u8] = b"MZ";

impl Code {
    /// The code of the ELF or PE file `bytes` hold.
    ///
    /// # Errors
    /// Where they are neither, are malformed, or are for a machine whose instructions are not
    /// read.
    pub fn read(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.starts_with(ELF) {
            let file = vun_elf::File::read(bytes).map_err(|e| Error::Malformed(e.to_string()))?;
            Self::of_elf(&file)
        } else if bytes.starts_with(DOS) {
            let image = vun_pe::Image::read(bytes).map_err(|e| Error::Malformed(e.to_string()))?;
            Self::of_pe(&image)
        } else {
            Err(Error::Format)
        }
    }
}
