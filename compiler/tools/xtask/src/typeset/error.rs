use std::fmt;
use std::io;
use std::str::Utf8Error;

use vun_json::Mismatch;
use vun_tool::Failure;

/// Why a document's variants could not be read.
#[derive(Debug)]
pub(super) enum Unread {
    Typst(Failure),
    Encoding(Utf8Error),
    Json(vun_json::Error),
    Shape(Mismatch),
}

/// Why a document could not be typeset.
#[derive(Debug)]
pub(super) enum Failed {
    Typst(Failure),
    Write(io::Error),
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Typst(_) => "typst could not evaluate them",
            Self::Encoding(_) => "typst printed them in something other than UTF-8",
            Self::Json(_) => "typst printed them as something other than JSON",
            Self::Shape(_) => "they are not an array of `(path: (..), inputs: (..))`",
        })
    }
}

impl std::error::Error for Unread {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Typst(e) => Some(e),
            Self::Encoding(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Shape(e) => Some(e),
        }
    }
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Typst(_) => "typst could not compile it",
            Self::Write(_) => "its output could not be written",
        })
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Typst(e) => Some(e),
            Self::Write(e) => Some(e),
        }
    }
}
