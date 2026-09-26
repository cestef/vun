//! Terminal output shared by every front end: output names what each piece of text is
//! ([`Style`]), one palette decides how it looks, and each stream decides whether it shows
//! colors ([`ColorChoice`], `--color`). Commands write [`Text`] through a [`Shell`]: results to
//! standard output, status lines and diagnostics to standard error.

mod color;
mod markdown;
mod pager;
mod shell;
mod style;
mod table;
#[cfg(test)]
mod tests;
mod text;
mod units;

pub use color::{Color, ColorChoice, Env, Stream};
pub use shell::{Out, Shell, Verb, Verbosity};
pub use style::Style;
pub use table::{Align, Column, Table};
pub use text::{Piece, Text};
pub use units::{Bytes, Counted, Elapsed, Noun};
