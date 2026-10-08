//! DWARF debugging information (version 4), written as sections of a relocatable object: the
//! line table of each function, which debuggers, profilers and `addr2line` read to map
//! addresses to source lines, the one compilation unit naming it with an entry per function
//! (its name, where it is declared and its code), and the call frame information unwinders read
//! (`.eh_frame`). Addresses are relocations
//! against the functions' symbols, so the linker places them.

mod frame;
mod info;
mod line;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::OsString;
use std::path::PathBuf;

use vun_object::{Object, SectionId, SymbolId};

pub use frame::{CallFrame, Fde, Step};

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

/// A function, whose code is `size` bytes at its symbol: its name as debuggers show it, the line
/// declaring it, and the rows of its code.
pub struct Function {
    pub symbol: SymbolId,
    pub size: u64,
    pub name: String,
    pub declared: Option<Declared>,
    pub rows: Vec<Row>,
}

/// Where a function is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declared {
    pub file: File,
    /// Counted from 1.
    pub line: u32,
}

/// The line information of an object's code, built function by function.
#[derive(Default)]
pub struct Lines {
    files: Vec<PathBuf>,
    /// Each of `files` by the bytes of its path: a row looks its file up by hash, as comparing
    /// or hashing a `Path` walks its components.
    numbers: HashMap<OsString, File, foldhash::fast::RandomState>,
    functions: Vec<Function>,
}

impl File {
    /// Its number in the line table, which counts from 1.
    pub(crate) fn number(self) -> u64 {
        u64::from(self.0) + 1
    }
}

impl Lines {
    /// The file at `path`, added the first time.
    ///
    /// # Panics
    /// Past 2^32 files.
    pub fn file(&mut self, path: PathBuf) -> File {
        match self.numbers.entry(path.into_os_string()) {
            Entry::Occupied(file) => *file.get(),
            Entry::Vacant(entry) => {
                let file = File(u32::try_from(self.files.len()).expect("fewer than 2^32 files"));
                self.files.push(entry.key().into());
                *entry.insert(file)
            }
        }
    }

    pub fn function(&mut self, function: Function) {
        self.functions.push(function);
    }

    /// Adds the debugging sections to `object`: the unit named `name`, made by `producer`,
    /// whose code is the section `text`, its functions and its line table. Nothing is added
    /// without rows.
    pub fn write(&self, object: &mut Object, text: SectionId, name: &str, producer: &str) {
        if self.functions.iter().all(|f| f.rows.is_empty()) {
            return;
        }
        let line = self.table(object);
        info::Unit { name, producer, text, line, functions: &self.functions }.write(object);
    }
}
