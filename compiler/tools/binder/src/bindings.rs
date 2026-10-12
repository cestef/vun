//! What binding a header gives, and why it can give nothing.

use std::path::PathBuf;

use vun_codec::{Decode, Encode};

/// The bindings of a header: the text of a Vun file, and what it leaves out. When the request
/// split them, the file spreads the `parts` and declares nothing itself.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Bindings {
    pub source: String,
    pub parts: Vec<Part>,
    pub skipped: Vec<Skipped>,
}

/// The bindings of the headers of one directory, a file beside the one that spreads it.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Part {
    /// The name of the file, without its extension, and of the namespace it makes.
    pub name: String,
    pub source: String,
}

/// A library a header's functions and variables are linked from.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Library {
    /// Its name, as the linker is given it: `gtk-4` for `-lgtk-4`.
    pub name: String,
    /// The directory of the headers it declares; every header when there is none. Of two libraries
    /// declaring a header, the one with the deeper directory is where it is linked from.
    pub headers: Option<PathBuf>,
}

impl Library {
    /// The library written `name`, or `name=dir` for the headers under `dir`.
    #[must_use]
    pub fn named(written: &str) -> Self {
        match written.split_once('=') {
            Some((name, dir)) => Self { name: name.to_owned(), headers: Some(PathBuf::from(dir)) },
            None => Self { name: written.to_owned(), headers: None },
        }
    }
}

/// A declaration left out, or bound without what it could not say, and why.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Skipped {
    pub name: String,
    pub file: PathBuf,
    pub line: u32,
    pub reason: Reason,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Reason {
    /// A type it uses has no Vun spelling.
    Unsupported(Unsupported),
    /// A struct bound as its size and alignment only, and why.
    Bytes(Bytes),
}

/// Why a type has no Vun spelling here.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Unsupported {
    /// A C type Vun has no counterpart for, as C spells it: `long double`, a union, `...`.
    Type(String),
    /// A struct bound without its fields, used by value where they matter: an incomplete one
    /// anywhere, one bound as bytes in a call, whose convention classifies the fields.
    Opaque(String),
}

/// Why a struct is bound as its size and alignment only.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Bytes {
    /// A union: its fields overlap.
    Union,
    /// The bit field of this name, whose layout C leaves to the compiler.
    BitField(String),
    /// The field of this name, whose type has no Vun spelling.
    Field(String),
}

/// Why no bindings were written.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Failure {
    /// The library the header is read through could not be loaded.
    Missing(Missing),
    /// The header could not be opened at all.
    Unreadable,
    /// The reader reported errors, which the bindings would inherit, as it worded them.
    Errors(Vec<String>),
}

/// Why the library a header is read through could not be loaded: its version must be `oldest`
/// or later.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Missing {
    pub oldest: u32,
    pub why: Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Unavailable {
    /// Not found or not openable, as the loader says.
    Unloadable(String),
    /// Older than the version the bindings need.
    Old,
}
