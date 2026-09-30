//! `vun ffi bind` (spec 07, section 5): Vun declarations from a C header, read through libclang
//! with the build's compile flags, so that sizes, macros and conditionals are the target's.
//!
//! Only what the header's own directory declares is bound; what it includes from elsewhere,
//! such as the system's headers, is reached through what it stands for (`size_t` is `u64` on a
//! 64-bit target). What Vun cannot say is left out and reported, never approximated.

mod emit;
mod items;
mod macros;
mod names;
mod types;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use vun_clang::{Clang, Cursor, Diagnostic, Missing, Severity};

pub use types::Unsupported;

/// The bindings of a header: the text of a Vun file, and what it leaves out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bindings {
    pub source: String,
    pub skipped: Vec<Skipped>,
}

/// A declaration left out, or bound without what it could not say, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub name: String,
    pub file: PathBuf,
    pub line: u32,
    pub reason: Reason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A type it uses has no Vun spelling.
    Unsupported(Unsupported),
    /// A function taking arguments past its parameters, `...`.
    Variadic,
    /// A struct bound as its size and alignment only, and why.
    Bytes(Bytes),
}

/// Why a struct is bound as its size and alignment only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bytes {
    /// A union: its fields overlap.
    Union,
    /// The bit field of this name, whose layout C leaves to the compiler.
    BitField(String),
    /// The field of this name, whose type has no Vun spelling.
    Field(String),
}

/// Why no bindings were written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// libclang could not be loaded.
    Missing(Missing),
    /// clang could not read the header at all.
    Unreadable,
    /// clang reported errors, which the bindings would inherit.
    Errors(Vec<Diagnostic>),
}

/// How much of a struct is bound when its fields are not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opacity {
    /// Its size and alignment, as private bytes: it can be a field, not an argument.
    Sized,
    /// Nothing: C declares no fields, so it is reached through pointers only.
    Incomplete,
}

/// A declaration's identity: a tag (`struct point`), an ordinary name (a typedef, a function),
/// or, for an anonymous struct, where it is written.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Tag(String),
    Ordinary(String),
    At(PathBuf, u32, u32),
}

/// What binding a unit has found so far.
pub(crate) struct Binder<'u> {
    /// The declarations bound, in source order.
    decls: Vec<Cursor<'u>>,
    /// The Vun name of each declaration bound.
    names: HashMap<Key, String>,
    opaque: HashMap<String, Opacity>,
    /// Names a binding may not take: those every file sees already, such as `type` and `test`,
    /// which Vun does not let a declaration shadow, and the bindings' own top-level names for
    /// parameters.
    reserved: HashSet<String>,
    /// The bindings' own top-level names, which parameters may not take.
    params_reserved: HashSet<String>,
    skipped: Vec<Skipped>,
}

impl Bindings {
    /// The bindings of `header`, parsed with the compile flags `args`, avoiding the names
    /// `reserved`, those the file the bindings are written to sees already.
    ///
    /// # Errors
    /// Returns why nothing could be bound: libclang is missing, or the header does not parse.
    pub fn of(header: &Path, args: &[String], reserved: &[String]) -> Result<Self, Failure> {
        let clang = Clang::load().map_err(Failure::Missing)?;
        let unit = clang.parse(header, args).ok_or(Failure::Unreadable)?;
        let errors: Vec<Diagnostic> =
            unit.diagnostics().into_iter().filter(|d| d.severity == Severity::Error).collect();
        if !errors.is_empty() {
            return Err(Failure::Errors(errors));
        }
        // The header's own declarations, and those of headers beside it: by absolute paths, as
        // `board.h` is beside `./other.h` but not `/usr/include/stdint.h`.
        let absolute = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
        let dir = absolute(header).parent().map(Path::to_path_buf).unwrap_or_default();
        let mine: Vec<Cursor<'_>> = unit
            .root()
            .children()
            .into_iter()
            .filter(|c| c.location().file.is_some_and(|f| absolute(&f).starts_with(&dir)))
            .collect();
        let mut binder = Binder {
            decls: Vec::new(),
            names: HashMap::new(),
            opaque: HashMap::new(),
            reserved: reserved.iter().cloned().collect(),
            params_reserved: HashSet::new(),
            skipped: Vec::new(),
        };
        binder.collect(&mine);
        let name = header.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let source = binder.file(&name).source();
        Ok(Bindings { source, skipped: binder.skipped })
    }
}

impl Binder<'_> {
    /// The Vun name `decl` is bound under.
    pub(crate) fn bound(&self, decl: &Cursor<'_>) -> Option<String> {
        self.names.get(&Self::key(decl)?).cloned()
    }

    /// The identity of a record, enum or typedef declaration.
    pub(crate) fn key(decl: &Cursor<'_>) -> Option<Key> {
        let name = decl.name();
        match decl.kind() {
            vun_clang::Kind::Typedef => Some(Key::Ordinary(name)),
            vun_clang::Kind::Struct | vun_clang::Kind::Union | vun_clang::Kind::Enum => {
                // Anonymous records are spelled `struct (unnamed at h.h:3:9)` by newer clangs.
                if decl.is_anonymous() || name.is_empty() || name.contains(' ') {
                    let at = decl.location();
                    Some(Key::At(at.file?, at.line, at.column))
                } else {
                    Some(Key::Tag(name))
                }
            }
            _ => None,
        }
    }

    pub(crate) fn skip(&mut self, decl: &Cursor<'_>, reason: Reason) {
        let at = decl.location();
        let name = decl.name();
        self.skipped.push(Skipped { name, file: at.file.unwrap_or_default(), line: at.line, reason });
    }
}

#[cfg(test)]
mod tests;
