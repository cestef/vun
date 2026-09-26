//! DWARF debugging information (version 4), written as sections of a relocatable object: the
//! line table of each function, which debuggers, profilers and `addr2line` read to map
//! addresses to source lines, and the one compilation unit naming it. Addresses are relocations
//! against the functions' symbols, so the linker places them.

mod encode;
mod info;
mod line;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use vun_object::{Object, SectionId, SymbolId};

/// A file of the line table, by its position there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct File(u32);

/// A row of a line table: code from `offset` in its function on comes from `line` of `file`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub offset: u64,
    pub file: File,
    /// Counted from 1.
    pub line: u32,
}

/// The rows of one function, whose code is `size` bytes at its symbol.
pub struct Sequence {
    pub symbol: SymbolId,
    pub size: u64,
    pub rows: Vec<Row>,
}

/// The line information of an object's code, built function by function.
#[derive(Default)]
pub struct Lines {
    files: Vec<PathBuf>,
    sequences: Vec<Sequence>,
}

impl Lines {
    /// The file at `path`, added the first time.
    ///
    /// # Panics
    /// Past 2^32 files.
    pub fn file(&mut self, path: PathBuf) -> File {
        let at = self.files.iter().position(|f| *f == path).unwrap_or_else(|| {
            self.files.push(path);
            self.files.len() - 1
        });
        File(u32::try_from(at).expect("fewer than 2^32 files"))
    }

    pub fn sequence(&mut self, sequence: Sequence) {
        self.sequences.push(sequence);
    }

    /// Adds the debugging sections to `object`: the unit named `name`, made by `producer`,
    /// whose code is the section `text`, and its line table. Nothing is added without rows.
    pub fn write(&self, object: &mut Object, text: SectionId, name: &str, producer: &str) {
        if self.sequences.iter().all(|s| s.rows.is_empty()) {
            return;
        }
        let line = self.table(object);
        info::Unit { name, producer, text, line }.write(object);
    }
}
