//! The path of a URI (RFC 3986, section 3.3), its segments, and the removal of its dot segments
//! (section 5.2.4).

use std::borrow::Cow;

use crate::parse::SLASH;
use crate::percent::Encoded;

/// A URI's path, as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path<'a>(pub(crate) Encoded<'a>);

const CURRENT: &str = ".";
const PARENT: &str = "..";

impl<'a> Path<'a> {
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0.as_str()
    }

    #[must_use]
    pub const fn encoded(self) -> Encoded<'a> {
        self.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// Whether it starts at the root, with `/`.
    #[must_use]
    pub fn is_absolute(self) -> bool {
        self.as_str().as_bytes().first() == Some(&SLASH)
    }

    /// Its segments, each as written: `/a/b/` is `a`, `b` and an empty last one.
    pub fn segments(self) -> impl DoubleEndedIterator<Item = Encoded<'a>> {
        let text = self.as_str();
        let text = text.strip_prefix(char::from(SLASH)).unwrap_or(text);
        let empty = self.is_empty();
        text.split(char::from(SLASH)).filter(move |_| !empty).map(Encoded)
    }

    /// The path without its dot segments, `.` and `..`, each `..` removing the segment before it.
    #[must_use]
    pub fn without_dots(self) -> Cow<'a, str> {
        Self::remove_dots(self.as_str())
    }

    /// RFC 3986's `remove_dot_segments`, step by step as section 5.2.4 writes it: the input read
    /// a segment at a time, each moved to the output unless it is a dot segment, `..` removing the
    /// output's last.
    pub(crate) fn remove_dots(path: &str) -> Cow<'_, str> {
        let dotted = |s: &str| s == CURRENT || s == PARENT;
        if !path.split(char::from(SLASH)).any(dotted) {
            return Cow::Borrowed(path);
        }
        let pop = |out: &mut String| out.truncate(out.rfind(char::from(SLASH)).unwrap_or(0));
        let mut input = path;
        let mut out = String::with_capacity(path.len());
        while !input.is_empty() {
            if let Some(rest) = input.strip_prefix("../").or_else(|| input.strip_prefix("./")) {
                input = rest;
            } else if input.starts_with("/./") {
                input = &input[2..];
            } else if input == "/." {
                input = "/";
            } else if input.starts_with("/../") {
                input = &input[3..];
                pop(&mut out);
            } else if input == "/.." {
                input = "/";
                pop(&mut out);
            } else if dotted(input) {
                input = "";
            } else {
                let from = usize::from(input.starts_with(char::from(SLASH)));
                let end = input[from..].find(char::from(SLASH)).map_or(input.len(), |p| from + p);
                out.push_str(&input[..end]);
                input = &input[end..];
            }
        }
        Cow::Owned(out)
    }

    /// RFC 3986's merge (section 5.2.3): `reference` read in the directory of `base`, a path of a
    /// URI that has an authority when `authority`.
    pub(crate) fn merge(base: &str, authority: bool, reference: &str) -> String {
        if authority && base.is_empty() {
            return format!("/{reference}");
        }
        let directory = base.rfind(char::from(SLASH)).map_or("", |at| &base[..=at]);
        format!("{directory}{reference}")
    }
}
