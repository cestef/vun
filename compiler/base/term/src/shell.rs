//! Where a command writes: results to standard output, progress and diagnostics to standard
//! error, as cargo's shell does. Each stream knows whether it shows colors.

use std::io::{self, Write};

use crate::styled::Lines;
use crate::{Bar, Color, ColorChoice, Env, Format, Live, Progress, Stream, Style, Text, Unit};

/// A stream and whether it shows colors. Plain writes pass through; [`Out::print`] styles.
/// Showing [`Format::Styled`], it writes every line as its pieces instead.
pub struct Out<'a> {
    w: &'a mut dyn Write,
    terminal: bool,
    color: Color,
    lines: Option<Lines>,
    pub(crate) progress: Progress,
}

impl<'a> Out<'a> {
    /// A stream that is not a terminal: plain, whatever `auto` would say.
    pub fn plain(w: &'a mut dyn Write) -> Self {
        Self { w, terminal: false, color: Color::Never, lines: None, progress: Progress::off() }
    }

    /// A stream shown on a terminal when `terminal`.
    pub fn new(w: &'a mut dyn Write, terminal: bool) -> Self {
        Self { w, terminal, color: Color::Never, lines: None, progress: Progress::off() }
    }

    /// A stream whose colors are already decided, as a renderer holding its choice writes.
    pub fn colored(w: &'a mut dyn Write, color: Color) -> Self {
        Self { w, terminal: false, color, lines: None, progress: Progress::off() }
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
        self
    }

    /// Writes `text`, styled when this stream shows colors.
    ///
    /// # Errors
    /// Fails when the stream does.
    pub fn print(&mut self, text: &Text) -> io::Result<()> {
        let progress = self.progress.clone();
        let mut held = progress.hold();
        self.emit(text)?;
        if let Some(held) = &mut held {
            held.ended_line(text.pieces().last().is_some_and(|p| p.text.ends_with('\n')));
        }
        Ok(())
    }

    /// Writes `text` and a newline.
    ///
    /// # Errors
    /// Fails when the stream does.
    pub fn line(&mut self, text: &Text) -> io::Result<()> {
        let progress = self.progress.clone();
        let mut held = progress.hold();
        self.emit(text)?;
        self.put(b"\n")?;
        if let Some(held) = &mut held {
            held.ended_line(true);
        }
        drop(held);
        progress.redraw();
        Ok(())
    }

    // Writes `text` where the bars are not drawn.
    fn emit(&mut self, text: &Text) -> io::Result<()> {
        if let Some(lines) = &mut self.lines {
            return lines.print(self.w, text);
        }
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

    // Writes bytes where the bars are not drawn.
    fn put(&mut self, buf: &[u8]) -> io::Result<()> {
        match &mut self.lines {
            Some(lines) => lines.write(self.w, buf),
            None => self.w.write_all(buf),
        }
    }

    fn choose(&mut self, choice: ColorChoice, env: Env) {
        self.color = choice.color(env, self.terminal);
    }

    // Shows `format` as the stream `stream`.
    fn show(&mut self, format: Format, stream: Stream) {
        self.lines = (format == Format::Styled).then(|| Lines::new(stream));
    }
}

impl Drop for Out<'_> {
    // A line of pieces begun and never ended is still written.
    fn drop(&mut self) {
        if let Some(lines) = &mut self.lines {
            let _ = lines.finish(self.w);
        }
    }
}

impl Write for Out<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // The bars are off the terminal while this writes.
        let progress = self.progress.clone();
        let mut held = progress.hold();
        let written = match &mut self.lines {
            Some(lines) => lines.write(self.w, buf).map(|()| buf.len()),
            None => self.w.write(buf),
        }?;
        if let (Some(held), Some(last)) = (&mut held, buf[..written].last()) {
            held.ended_line(*last == b'\n');
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

/// How much a command says about what it does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    /// Results and diagnostics only.
    Quiet,
    /// Status lines as well.
    #[default]
    Normal,
    /// A status line for each step as well (`-v`), such as each file checked.
    Verbose,
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

    /// Whether it reports something that failed.
    #[must_use]
    pub fn is_failure(self) -> bool {
        self.style == Style::Error
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
    format: Format,
    env: Env,
    progress: Progress,
}

impl<'a> Shell<'a> {
    /// A shell over `out` and `err`, colored as `auto` decides for each.
    #[must_use]
    pub fn new(out: Out<'a>, err: Out<'a>, env: Env) -> Self {
        let mut shell = Self {
            out,
            err,
            verbosity: Verbosity::Normal,
            format: Format::Human,
            env,
            progress: Progress::off(),
        };
        shell.choose(ColorChoice::Auto);
        shell
    }

    /// A shell writing plain text to `out` and `err`, as tests read it.
    pub fn plain(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self {
            out: Out::plain(out),
            err: Out::plain(err),
            verbosity: Verbosity::Normal,
            format: Format::Human,
            env: Env::default(),
            progress: Progress::off(),
        }
    }

    #[must_use]
    pub const fn format(&self) -> Format {
        self.format
    }

    /// Shows records in `format` on both streams.
    pub fn show(&mut self, format: Format) {
        self.format = format;
        self.out.show(format, Stream::Stdout);
        self.err.show(format, Stream::Stderr);
    }

    /// Applies a `--color` choice to both streams.
    pub fn choose(&mut self, choice: ColorChoice) {
        self.out.choose(choice, self.env);
        self.err.choose(choice, self.env);
    }

    /// Draws the bars of tasks on standard error, when that is a terminal showing people's
    /// output: after the options deciding that are applied. Not drawn when the logs are on,
    /// which they would tear.
    pub fn animate(&mut self, logging: bool) {
        let shown = self.format == Format::Human && self.verbosity != Verbosity::Quiet && !logging;
        if !(shown && self.err.is_terminal()) {
            return;
        }
        let width =
            std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()).unwrap_or(crate::DEFAULT_WIDTH);
        self.progress = Progress::drawn(Box::new(io::stderr()), self.err.color(), width);
        self.err.progress = self.progress.clone();
        self.progress.make_ambient();
        // Results written to a terminal's standard output share the screen with the bars.
        if self.out.is_terminal() {
            self.out.progress = self.progress.clone();
        }
    }

    /// A bar for `total` units of work (0 when not known), drawn if bars are.
    #[must_use]
    pub fn bar(&self, verb: Verb, subject: Text, total: u64, unit: Unit) -> Bar {
        self.progress.bar(verb, subject, total, unit)
    }

    /// A line saying what `live` reports, from `delay` after it is made, drawn if bars are.
    #[must_use]
    pub fn watch(&self, verb: Verb, delay: std::time::Duration, live: Live) -> Bar {
        self.progress.watch(verb, delay, live)
    }

    /// The progress, to hand to threads that advance bars.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.progress.clone()
    }
}
