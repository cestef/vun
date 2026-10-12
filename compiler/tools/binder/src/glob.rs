//! The names a binding is asked for: text with `*` standing for any run of characters.

use vun_codec::{Decode, Encode};

/// A name pattern: `gtk_window_*` names every `gtk_window_` and whatever follows.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Glob(String);

/// What stands for any run of characters, a possibly empty one.
const ANY: char = '*';

impl Glob {
    #[must_use]
    pub fn new(pattern: &str) -> Self {
        Self(pattern.to_owned())
    }

    /// The pattern as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `name` is the whole of the pattern, each `*` for some run of its characters.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        let mut pieces = self.0.split(ANY);
        let first = pieces.next().unwrap_or_default();
        let Some(mut rest) = name.strip_prefix(first) else { return false };
        let mut pieces: Vec<&str> = pieces.collect();
        let Some(last) = pieces.pop() else { return rest.is_empty() };
        for piece in pieces {
            let Some(at) = rest.find(piece) else { return false };
            rest = &rest[at + piece.len()..];
        }
        rest.ends_with(last)
    }
}
