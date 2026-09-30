//! Source files, file ids and byte spans.
//!
//! A [`Span`] is a half-open byte range inside one source file. Spans never cross files; anything
//! that relates several locations does so through origin chains (`vun-origin`).

use std::hash::{Hash, Hasher};

use vun_codec::Encode;

mod codec;
mod data;
mod key;

pub use key::{FileRef, PackageId, Version};

/// The extension of Vun source files, as a literal for [`source_file!`].
#[macro_export]
macro_rules! extension {
    () => {
        "vun"
    };
}

/// The extension of Vun source files.
pub const EXTENSION: &str = extension!();

/// The name of the source file `stem` with the extension: `source_file!("build")` is `build.vun`.
#[macro_export]
macro_rules! source_file {
    ($stem:literal) => {
        concat!($stem, ".", $crate::extension!())
    };
}

/// A source file within one compilation session. It hashes by its key, the [`FileRef`] naming it
/// the same way in every process, so that hashes of spans and of anything holding them do not
/// depend on the order files were read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileId {
    index: u32,
    key: u64,
}

impl FileId {
    /// What names the file in every process, hashed.
    #[must_use]
    pub fn key(self) -> u64 {
        self.key
    }
}

impl Hash for FileId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.key);
    }
}

/// A half-open byte range `lo..hi`, relative to some text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode)]
pub struct ByteRange {
    pub lo: u32,
    pub hi: u32,
}

impl ByteRange {
    /// # Panics
    /// Panics if `lo > hi`.
    #[must_use]
    pub const fn new(lo: u32, hi: u32) -> Self {
        assert!(lo <= hi, "range starts after it ends");
        Self { lo, hi }
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.hi - self.lo
    }

    /// Whether the two share a byte; an empty range shares none.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.lo < other.hi && other.lo < self.hi
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.lo == self.hi
    }

    /// The smallest range covering both.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        Self { lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
    }

    /// Whether `other` lies within this range.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.lo <= other.lo && other.hi <= self.hi
    }

    /// The text this range selects.
    #[must_use]
    pub fn of(self, text: &str) -> &str {
        &text[self.lo as usize..self.hi as usize]
    }
}

/// A byte range in one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode)]
pub struct Span {
    pub file: FileId,
    pub range: ByteRange,
}

impl Span {
    /// # Panics
    /// Panics if `lo > hi`.
    #[must_use]
    pub const fn new(file: FileId, lo: u32, hi: u32) -> Self {
        Self { file, range: ByteRange::new(lo, hi) }
    }

    /// The smallest span covering both.
    ///
    /// # Panics
    /// Panics if the spans are in different files.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        assert_eq!(self.file, other.file, "cannot cover spans from different files");
        Self { file: self.file, range: self.range.cover(other.range) }
    }
}

/// A one-based line and column, the column counted both in bytes and in characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
    pub chars: u32,
}

/// A source file and the byte offset of each of its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// As shown to users.
    pub path: String,
    pub text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    /// # Panics
    /// Panics if the text is 4 GiB or larger.
    #[must_use]
    pub fn new(path: String, text: String) -> Self {
        let newlines = text.match_indices('\n').map(|(i, _)| i + 1);
        let line_starts = std::iter::once(0)
            .chain(newlines)
            .map(|i| u32::try_from(i).expect("source files are smaller than 4 GiB"))
            .collect();
        Self { path, text, line_starts }
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc, reason = "line indices fit u32 because offsets do")]
    pub fn line_col(&self, offset: u32) -> LineCol {
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let start = self.line_starts[line];
        let before = ByteRange::new(start, offset).of(&self.text);
        LineCol {
            line: u32::try_from(line + 1).expect("line numbers fit u32"),
            col: offset - start + 1,
            chars: u32::try_from(before.chars().count() + 1).expect("columns fit u32 because offsets do"),
        }
    }

    /// Each line's text and range, without its line break.
    ///
    /// # Panics
    /// Panics if the text is 4 GiB or larger, which [`SourceFile::new`] rejects.
    pub fn lines(&self) -> impl Iterator<Item = (&str, ByteRange)> {
        let ends = self.line_starts.iter().skip(1).copied();
        let len = u32::try_from(self.text.len()).expect("source files are smaller than 4 GiB");
        let ranges = self.line_starts.iter().zip(ends.chain([len])).map(|(&lo, hi)| ByteRange::new(lo, hi));
        ranges.filter(|r| !r.is_empty()).map(|r| {
            let line = r.of(&self.text).trim_end_matches(['\n', '\r']);
            let hi = r.lo + u32::try_from(line.len()).expect("lines are shorter than their file");
            (line, ByteRange::new(r.lo, hi))
        })
    }
}

/// Read access to source files.
pub trait SourceMap {
    fn file(&self, id: FileId) -> &SourceFile;

    /// Where `span` starts, as shown to users.
    fn locate(&self, span: Span) -> Location<'_> {
        let file = self.file(span.file);
        Location { path: &file.path, at: file.line_col(span.range.lo) }
    }
}

/// A position as shown to users; displays as `path:line:column`, the column in characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    pub path: &'a str,
    pub at: LineCol,
}

impl std::fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.path, self.at.line, self.at.chars)
    }
}

/// An in-memory set of source files, appended to through a shared reference, so that every
/// query of a compilation adds to one table.
#[derive(Default)]
pub struct SourceFiles {
    /// Each file, with the key naming it.
    files: boxcar::Vec<(FileRef, SourceFile)>,
    /// Each file by its key, the path it is shown as and the hash of its text, so that adding
    /// it again returns it.
    added: std::sync::RwLock<std::collections::HashMap<(FileRef, String, vun_hash::Hash), FileId>>,
    /// The first file added under each key with each text, whatever its path.
    texts: std::sync::RwLock<std::collections::HashMap<(FileRef, vun_hash::Hash), FileId>>,
}

impl SourceFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file outside any package, keyed by its path.
    pub fn add(&self, path: impl Into<String>, text: impl Into<String>) -> FileId {
        let path = path.into();
        self.add_as(FileRef::loose(&path), path, text)
    }

    /// Adds a file under `key`, which names it the same way in every process, shown as `path`;
    /// the same key, path and text is the file already added.
    ///
    /// # Panics
    /// Panics past `u32::MAX` files, or if a thread panicked while adding.
    pub fn add_as(&self, key: FileRef, path: impl Into<String>, text: impl Into<String>) -> FileId {
        let (path, text) = (path.into(), text.into());
        let (named, short) = (key.clone(), vun_hash::Stable::of(&key));
        let identity = (key, path, vun_hash::Hash::of(text.as_bytes()));
        if let Some(&file) = self.added.read().expect("source files").get(&identity) {
            return file;
        }
        let mut added = self.added.write().expect("source files");
        let (path, hash) = (identity.1.clone(), identity.2);
        let file = *added.entry(identity).or_insert_with(|| {
            vun_trace::tracing::trace!(key = ?named, path, text = %hash, "file");
            let index = self.files.push((named.clone(), SourceFile::new(path, text)));
            FileId { index: u32::try_from(index).expect("fewer than 2^32 files"), key: short }
        });
        self.texts.write().expect("source files").entry((named, hash)).or_insert(file);
        file
    }

    /// The file added under `key` with `text`, if one was.
    ///
    /// # Panics
    /// Panics if a thread panicked while adding.
    #[must_use]
    pub fn read(&self, key: &FileRef, text: &str) -> Option<FileId> {
        let texts = self.texts.read().expect("source files");
        texts.get(&(key.clone(), vun_hash::Hash::of(text.as_bytes()))).copied()
    }
}

impl SourceFiles {
    /// The files of the process, which file handles in interned values and saved memos are
    /// into, shared by every compilation in it.
    #[must_use]
    pub fn global() -> &'static Self {
        static FILES: std::sync::LazyLock<SourceFiles> = std::sync::LazyLock::new(SourceFiles::default);
        &FILES
    }

    /// The key naming `id` in every process.
    #[must_use]
    pub fn key(&self, id: FileId) -> &FileRef {
        &self.files[id.index as usize].0
    }
}

impl SourceMap for SourceFiles {
    fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.index as usize].1
    }
}

#[cfg(test)]
mod tests;
