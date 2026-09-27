//! Documents named by `file://` URIs (RFC 8089): an absolute path, each byte outside the
//! unreserved characters and `/` escaped as `%XX`.

use std::fmt::{self, Write};
use std::path::{Path, PathBuf};

/// What a URI naming a local file starts with.
const FILE: &str = "file://";
/// The bytes a path keeps as they are in a URI: RFC 3986's unreserved characters, and `/`.
const KEPT: &[u8] = b"-._~/";

/// A document's URI, as the client names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Uri(String);

impl Uri {
    pub(crate) fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// The URI of the local file `path`.
    pub(crate) fn of(path: &Path) -> Self {
        let mut uri = String::from(FILE);
        for &b in path.as_os_str().as_encoded_bytes() {
            if b.is_ascii_alphanumeric() || KEPT.contains(&b) {
                uri.push(char::from(b));
            } else {
                let _ = write!(uri, "%{b:02X}");
            }
        }
        Self(uri)
    }

    /// The local file it names; `None` for another scheme or a malformed escape.
    pub(crate) fn path(&self) -> Option<PathBuf> {
        let escaped = self.0.strip_prefix(FILE)?.as_bytes();
        let mut bytes = Vec::with_capacity(escaped.len());
        let mut rest = escaped.iter();
        while let Some(&b) = rest.next() {
            if b == b'%' {
                let (hi, lo) =
                    (char::from(*rest.next()?).to_digit(16)?, char::from(*rest.next()?).to_digit(16)?);
                bytes.push(u8::try_from(hi * 16 + lo).ok()?);
            } else {
                bytes.push(b);
            }
        }
        String::from_utf8(bytes).ok().map(PathBuf::from)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
