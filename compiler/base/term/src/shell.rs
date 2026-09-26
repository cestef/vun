//! Where a command writes: results to standard output, progress and diagnostics to standard
//! error, as cargo's shell does. Each stream knows whether it shows colors.

use std::io::{self, Write};

use crate::{Color, ColorChoice, Env, Style, Text};

/// A stream and whether it shows colors. Plain writes pass through; [`Out::print`] styles.
pub struct Out<'a> {
    w: &'a mut dyn Write,
    terminal: bool,
    color: Color,
}

impl<'a> Out<'a> {
    /// A stream that is not a terminal: plain, whatever `auto` would say.
    pub fn plain(w: &'a mut dyn Write) -> Self {
        Self { w, terminal: false, color: Color::Never }
    }

    /// A stream shown on a terminal when `terminal`.
    pub fn new(w: &'a mut dyn Write, terminal: bool) -> Self {
        Self { w, terminal, color: Color::Never }
    }

    /// A stream whose colors are already decided, as a renderer holding its choice writes.
    pub fn colored(w: &'a mut dyn Write, color: Color) -> Self {
        Self { w, terminal: false, color }
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    #[must_use]
    pub fn color(&self) -> Color {
        self.color
    }

    /// The writer, for renderers that style their own output.
    pub fn writer(&mut self) -> &mut dyn Write {
        self.w
    }

    /// Writes `text`, styled when this stream shows colors.
    ///
    /// # Errors
    /// Fails when the stream does.
    pub fn print(&mut self, text: &Text) -> io::Result<()> {
        for piece in text.pieces() {
            match (piece.style, self.color) {
                (Some(style), Color::Always) => {
                    let ansi = style.ansi();
                    write!(self.w, "{}{}{}", ansi.render(), piece.text, ansi.render_reset())?;
                }
                _ => self.w.write_all(piece.text.as_bytes())?,
            }
        }
        Ok(())
    }

    /// Writes `text` and a newline.
    ///
    /// # Errors
    /// Fails when the stream does.
    pub fn line(&mut self, text: &Text) -> io::Result<()> {
        self.print(text)?;
        self.w.write_all(b"\n")
    }

    fn choose(&mut self, choice: ColorChoice, env: Env) {
        self.color = choice.color(env, self.terminal);
    }
}

impl Write for Out<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

/// How much a command says about what it does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verbosity {
    /// Results and diagnostics only.
    Quiet,
    /// Status lines as well.
    #[default]
    Normal,
}

/// A status line's verb, right-aligned in a column as cargo's `Compiling` and `Finished` are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verb {
    pub word: &'static str,
    pub style: Style,
}

impl Verb {
    /// The width of the verb column.
    pub const WIDTH: usize = 12;

    #[must_use]
    pub const fn good(word: &'static str) -> Self {
        Self { word, style: Style::Good }
    }

    #[must_use]
    pub const fn failed(word: &'static str) -> Self {
        Self { word, style: Style::Error }
    }

    #[must_use]
    pub const fn warned(word: &'static str) -> Self {
        Self { word, style: Style::Warning }
    }

    /// A line under a status line, aligned past the verb column.
    #[must_use]
    pub fn under(text: Text) -> Text {
        Text::new().pad(Self::WIDTH + 1).append(text)
    }

    /// The line `verb what`, the verb right-aligned.
    #[must_use]
    pub fn line(self, what: Text) -> Text {
        let room = Self::WIDTH.saturating_sub(self.word.chars().count());
        Text::new().pad(room).styled(self.style, self.word).plain(" ").append(what)
    }
}

pub struct Shell<'a> {
    pub out: Out<'a>,
    pub err: Out<'a>,
    pub verbosity: Verbosity,
    env: Env,
}

impl<'a> Shell<'a> {
    /// A shell over `out` and `err`, colored as `auto` decides for each.
    #[must_use]
    pub fn new(out: Out<'a>, err: Out<'a>, env: Env) -> Self {
        let mut shell = Self { out, err, verbosity: Verbosity::Normal, env };
        shell.choose(ColorChoice::Auto);
        shell
    }

    /// A shell writing plain text to `out` and `err`, as tests read it.
    pub fn plain(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self { out: Out::plain(out), err: Out::plain(err), verbosity: Verbosity::Normal, env: Env::default() }
    }

    /// Applies a `--color` choice to both streams.
    pub fn choose(&mut self, choice: ColorChoice) {
        self.out.choose(choice, self.env);
        self.err.choose(choice, self.env);
    }

    /// Reports progress on standard error, unless quiet. Best-effort: a closed stream does not
    /// fail the command.
    pub fn status(&mut self, verb: Verb, what: Text) {
        if self.verbosity == Verbosity::Normal {
            let _ = self.err.line(&verb.line(what));
        }
    }
}
