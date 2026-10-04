//! Writing a built program to its device (spec 12, section 2). A device taking blocks over USB
//! mass storage is given a file of them (`std.build.Flash.uf2`), made beside the program from the
//! bytes its loadable segments put in memory and where they put them.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_elf::{File, Malformed};
use vun_fs::Edit;
use vun_origin::OriginId;
use vun_uf2::{Piece, Uf2, Unaddressable};

use crate::Session;

/// The extension of a file of blocks.
const EXTENSION: &str = "uf2";

/// Why a program is not written as a file of blocks.
#[derive(Debug)]
enum Unwritten {
    Read(io::Error),
    Elf(Malformed),
    Address(Unaddressable),
    Write(io::Error),
}

impl fmt::Display for Unwritten {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read(_) => "the program cannot be read",
            Self::Elf(_) => "the program is not an ELF file the toolchain reads",
            Self::Address(_) => "a segment of the program lies past what a block addresses",
            Self::Write(_) => "the file cannot be written",
        })
    }
}

impl std::error::Error for Unwritten {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) | Self::Write(e) => Some(e),
            Self::Elf(e) => Some(e),
            Self::Address(e) => Some(e),
        }
    }
}

impl Unwritten {
    /// The file of blocks for `uf2` made of the program at `program`, written beside it, returned
    /// where it is.
    fn file(uf2: Uf2, program: &Path) -> Result<PathBuf, Self> {
        let bytes = std::fs::read(program).map_err(Self::Read)?;
        let loads = File::loads(&bytes).map_err(Self::Elf)?;
        let pieces: Vec<Piece<'_>> =
            loads.iter().map(|load| Piece { address: load.physical, bytes: load.bytes }).collect();
        let blocks = uf2.write(&pieces).map_err(Self::Address)?;
        let file = program.with_extension(EXTENSION);
        file.put(blocks).map_err(Self::Write)?;
        Ok(file)
    }
}

impl Session {
    /// Writes the program built at `program` as a file of blocks for the chip family `family`,
    /// beside it, and returns the file; what stopped it is reported at `at`, the plan.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    pub(crate) fn write_uf2(
        &mut self,
        program: &Path,
        family: u32,
        at: OriginId,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        Unwritten::file(Uf2 { family }, program).map_err(|why| {
            let msg =
                Msg::new().text("cannot write ").code(program.display().to_string()).text(" as a UF2 file");
            self.diag.error(Diagnostic::error(errors::PLAN_UNFLASHED, msg).at(at, None).caused_by(&why))
        })
    }
}
