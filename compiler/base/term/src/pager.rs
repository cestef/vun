//! Long output through a pager when standard output is a terminal, as git and jj page.

use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::{Format, Out, Shell, Text};

/// The variable naming the pager, with its arguments.
const PAGER: &str = "PAGER";
/// The pager without one: quit when the text fits the screen, keep colors, leave the screen.
const DEFAULT: [&str; 2] = ["less", "-FRX"];

impl Shell<'_> {
    /// Shows `text` through the pager when standard output is a terminal shown for people;
    /// prints it otherwise, or when no pager starts.
    ///
    /// # Errors
    /// Fails when the output does.
    pub fn page(&mut self, text: &Text) -> io::Result<()> {
        if !self.out.is_terminal() || self.format() == Format::Styled {
            return self.out.print(text);
        }
        let named = std::env::var(PAGER).ok().filter(|p| !p.trim().is_empty());
        let words: Vec<String> = named.map_or_else(
            || DEFAULT.map(str::to_owned).to_vec(),
            |p| p.split_whitespace().map(str::to_owned).collect(),
        );
        let Some((program, flags)) = words.split_first() else { return self.out.print(text) };
        let Ok(mut child) = Command::new(program).args(flags).stdin(Stdio::piped()).spawn() else {
            return self.out.print(text);
        };
        if let Some(mut stdin) = child.stdin.take() {
            // A pager quit early closes its input; that is not a failure.
            let _ = Out::colored(&mut stdin, self.out.color()).print(text);
            let _ = stdin.flush();
        }
        child.wait().map(drop)
    }
}
