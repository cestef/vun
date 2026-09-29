//! The cases names are written in (spec 10, section 5), for tools that write names of their own
//! from others: a binding of a C name, a Rust spelling of a grammar name.

#[cfg(test)]
mod tests;

/// Joins the words of a `snake_case` name, and marks a private one in front.
const SEPARATOR: char = '_';

/// How the words of a name are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// `RingBuffer`: each word capitalized, joined.
    Pascal,
    /// `write_all`: lowercase words joined by `_`.
    Snake,
}

impl Case {
    /// `name` written in this case, its leading `_` kept. Words are split at `_`, before an
    /// uppercase letter following a lowercase one or a digit, and before the last letter of a run
    /// of uppercase ones followed by a lowercase one: `HTTPServer` is `http_server`, `color_t` is
    /// `ColorT`, `MODE_READ` is `ModeRead`.
    #[must_use]
    pub fn write(self, name: &str) -> String {
        let words = name.trim_start_matches(SEPARATOR);
        let mut out = name[..name.len() - words.len()].to_owned();
        for (i, word) in Self::words(words).into_iter().enumerate() {
            match self {
                Self::Pascal => {
                    let mut chars = word.chars();
                    out.extend(chars.next().map(|c| c.to_ascii_uppercase()));
                    out.extend(chars.map(|c| c.to_ascii_lowercase()));
                }
                Self::Snake => {
                    if i > 0 {
                        out.push(SEPARATOR);
                    }
                    out.push_str(&word.to_ascii_lowercase());
                }
            }
        }
        out
    }

    /// The words of `name`, as [`Self::write`] splits them.
    fn words(name: &str) -> Vec<&str> {
        let chars: Vec<(usize, char)> = name.char_indices().collect();
        let mut words = Vec::new();
        let mut start = 0;
        for (i, &(at, c)) in chars.iter().enumerate() {
            let before = i.checked_sub(1).map(|p| chars[p].1);
            let after = chars.get(i + 1).map(|&(_, c)| c);
            let boundary = c == SEPARATOR
                || c.is_ascii_uppercase()
                    && before.is_some_and(|b| {
                        b.is_ascii_lowercase()
                            || b.is_ascii_digit()
                            || b.is_ascii_uppercase() && after.is_some_and(|a| a.is_ascii_lowercase())
                    });
            if boundary {
                words.push(&name[start..at]);
                start = if c == SEPARATOR { at + c.len_utf8() } else { at };
            }
        }
        words.push(&name[start..]);
        words.retain(|w| !w.is_empty());
        words
    }
}
