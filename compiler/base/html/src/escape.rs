//! Escaping: the characters that would end text or an attribute value, replaced by references.
//! One table serves the renderer and the compiler baking a template.

/// Where text is written, which decides what must be escaped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escape {
    /// Element content: `&`, `<` and `>`.
    Text,
    /// A quoted attribute value: `&`, `<`, `>` and `"`.
    Attribute,
}

impl Escape {
    /// What `byte` is written as here, when it is escaped.
    #[must_use]
    pub const fn replacement(self, byte: u8) -> Option<&'static str> {
        match (byte, self) {
            (b'&', _) => Some("&amp;"),
            (b'<', _) => Some("&lt;"),
            (b'>', _) => Some("&gt;"),
            (b'"', Self::Attribute) => Some("&quot;"),
            _ => None,
        }
    }

    /// How many bytes `text` takes once escaped.
    #[must_use]
    pub const fn len(self, text: &str) -> usize {
        let bytes = text.as_bytes();
        let (mut at, mut len) = (0, 0);
        while at < bytes.len() {
            len += match self.replacement(bytes[at]) {
                Some(with) => with.len(),
                None => 1,
            };
            at += 1;
        }
        len
    }

    /// Appends `text` to `out`, escaped: runs needing nothing are copied whole.
    pub fn write(self, out: &mut String, text: &str) {
        let mut from = 0;
        for (at, byte) in text.bytes().enumerate() {
            if let Some(with) = self.replacement(byte) {
                out.push_str(&text[from..at]);
                out.push_str(with);
                from = at + 1;
            }
        }
        out.push_str(&text[from..]);
    }

    /// Appends `text`, already escaped as `from`, to `out`, escaping what this escapes beyond it.
    pub fn widen(self, from: Self, out: &mut String, text: &str) {
        let mut rest = text;
        while let Some(at) =
            rest.bytes().position(|b| from.replacement(b).is_none() && self.replacement(b).is_some())
        {
            out.push_str(&rest[..at]);
            out.push_str(self.replacement(rest.as_bytes()[at]).unwrap_or_default());
            rest = &rest[at + 1..];
        }
        out.push_str(rest);
    }
}
