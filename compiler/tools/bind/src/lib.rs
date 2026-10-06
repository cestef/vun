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

use vun_binder::{Bindings, Failure, Missing, Request, Skipped, Unavailable};
use vun_clang::{Clang, Cursor, Severity};

pub use vun_binder::{Bytes, Reason, Unsupported};

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

/// The C binder, which reads a header through libclang.
pub struct CBinder;

impl vun_binder::Binder for CBinder {
    fn name(&self) -> &'static str {
        "c"
    }

    /// Fails when libclang is missing, or the header does not parse.
    fn bind(&self, request: &Request) -> Result<Bindings, Failure> {
        let Request { header, args, reserved } = request;
        let clang = Clang::load().map_err(|missing| Failure::Missing(Self::missing(missing)))?;
        let unit = clang.parse(header, args).ok_or(Failure::Unreadable)?;
        let errors: Vec<String> = unit
            .diagnostics()
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.text)
            .collect();
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

impl CBinder {
    fn missing(missing: vun_clang::Missing) -> Missing {
        let why = match missing {
            vun_clang::Missing::Unloadable(why) => Unavailable::Unloadable(why),
            vun_clang::Missing::Old => Unavailable::Old,
        };
        Missing { oldest: vun_clang::OLDEST, why }
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
