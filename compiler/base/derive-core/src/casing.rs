//! How derives case the names of fields and variants where they are written: split into words at `_` and where a
//! lowercase letter or digit meets an uppercase one, then joined.

/// A casing, as a helper attribute's `case = ...` names it.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Casing {
    /// `text_document`
    #[default]
    Snake,
    /// `textDocument`
    Camel,
}

impl Casing {
    pub const NAMES: [(Self, &'static str); 2] = [(Self::Snake, "snake"), (Self::Camel, "camel")];

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(_, n)| *n == name).map(|&(c, _)| c)
    }

    /// `name`, a Rust identifier, in this casing.
    #[must_use]
    pub fn apply(self, name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for (i, word) in Self::words(name).into_iter().enumerate() {
            match self {
                Self::Snake => {
                    if i > 0 {
                        out.push('_');
                    }
                    out.push_str(&word);
                }
                Self::Camel => {
                    let mut chars = word.chars();
                    if let Some(first) = chars.next() {
                        if i == 0 {
                            out.push(first);
                        } else {
                            out.extend(first.to_uppercase());
                        }
                        out.push_str(chars.as_str());
                    }
                }
            }
        }
        out
    }

    /// The words of `name`, in lowercase.
    fn words(name: &str) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        let mut previous: Option<char> = None;
        for c in name.trim_start_matches("r#").chars() {
            let starts = previous.is_none_or(|p| p == '_' || (c.is_uppercase() && !p.is_uppercase()));
            if c != '_' {
                if starts || words.is_empty() {
                    words.push(String::new());
                }
                words.last_mut().expect("a word").extend(c.to_lowercase());
            }
            previous = Some(c);
        }
        words
    }
}
