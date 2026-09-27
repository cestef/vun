//! Listings: what a function's code holds, instruction by instruction, with its offsets and
//! bytes, as `vun debug code` prints it.

use std::fmt;

use crate::Label;
use crate::buffer::Code;

/// A line of a listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line<T> {
    /// An instruction, `len` bytes from `at`.
    Inst { at: u64, len: u32, inst: T },
    /// A label bound where the next instruction goes.
    Label(Label),
}

/// The bytes a line leaves room for before the instruction's text, so that most align; a longer
/// instruction's pushes its text right.
const BYTES_COLUMN: usize = 10;

impl<T: fmt::Display> Line<T> {
    /// The line as a listing prints it, reading its bytes from `code`.
    #[must_use]
    pub fn show<'a>(&'a self, code: &'a [u8]) -> impl fmt::Display + 'a {
        Shown { line: self, code }
    }
}

struct Shown<'a, T> {
    line: &'a Line<T>,
    code: &'a [u8],
}

impl<T: fmt::Display> fmt::Display for Shown<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Line::Label(label) => write!(f, "{label}:"),
            Line::Inst { at, len, inst } => {
                let from = usize::try_from(*at).expect("code fits memory");
                write!(f, "  {at:6x}  ")?;
                let bytes = &self.code[from..from + *len as usize];
                for (i, b) in bytes.iter().enumerate() {
                    write!(f, "{}{b:02x}", if i == 0 { "" } else { " " })?;
                }
                let pad = (BYTES_COLUMN * 3).saturating_sub(bytes.len() * 3);
                write!(f, "{:pad$} {inst}", "")
            }
        }
    }
}

/// The listing of a function's code, printable.
pub struct Listing<'a, T, S>(pub &'a Code<T, S>);

impl<T: fmt::Display, S> fmt::Display for Listing<'_, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.0.listing {
            writeln!(f, "{}", line.show(&self.0.bytes))?;
        }
        Ok(())
    }
}
