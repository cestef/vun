//! The sections of memory segments go in, as `wasm-ld` merges them: every `.rodata.*` segment in
//! `.rodata`, `.data.*` in `.data`, `.bss.*` in `.bss`, and a section a source names on its own,
//! in memory in that order with the named ones between the data and the zeros, so the zeros end
//! the memory and are never written.

/// A section of the program's memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Section<'o> {
    Rodata,
    Data,
    /// A section a source names, which the linker gives bounds.
    Named(&'o str),
    Bss,
}

/// The prefix of each merged section's segments' names, by the section.
const MERGED: [(&str, Section<'static>); 3] =
    [(".rodata", Section::Rodata), (".data", Section::Data), (".bss", Section::Bss)];

impl<'o> Section<'o> {
    /// The section the segment named `name` goes in.
    pub(crate) fn of(name: &'o str) -> Self {
        let merged = MERGED.iter().find(|(prefix, _)| {
            name.strip_prefix(prefix).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        });
        match merged {
            Some(&(_, section)) => section,
            None if name.is_empty() => Self::Data,
            None => Self::Named(name),
        }
    }

    /// The name a source gives it, for its bounds.
    pub(crate) const fn named(self) -> Option<&'o str> {
        match self {
            Self::Named(name) => Some(name),
            _ => None,
        }
    }
}
