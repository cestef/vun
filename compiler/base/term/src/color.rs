//! Whether a stream shows colors: the choice given with `--color`, and the environment deciding
//! `auto` by the conventions cargo and jj follow (<https://no-color.org>, <https://bixense.com/clicolors>).

use std::io::IsTerminal;

/// Whether a stream is written with colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Never,
    Always,
}

/// What `--color` asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorChoice {
    /// Colors on a terminal, unless the environment turns them off or forces them.
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Always, Self::Never];

    /// How the choice is written on the command line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    /// The choice `name` writes, `auto` when it writes none.
    #[must_use]
    pub fn named(name: Option<&str>) -> Self {
        Self::ALL.into_iter().find(|c| Some(c.name()) == name).unwrap_or_default()
    }

    /// The colors of a stream, given whether it is a terminal.
    #[must_use]
    pub fn color(self, env: Env, terminal: bool) -> Color {
        match self {
            Self::Always => Color::Always,
            Self::Never => Color::Never,
            Self::Auto => env.0.unwrap_or(if terminal { Color::Always } else { Color::Never }),
        }
    }
}

/// What the environment decides for `auto`, read once: nothing, or colors forced on or off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Env(pub Option<Color>);

impl Env {
    const NO_COLOR: &'static str = "NO_COLOR";
    const CLICOLOR_FORCE: &'static str = "CLICOLOR_FORCE";
    const CLICOLOR: &'static str = "CLICOLOR";
    const TERM: &'static str = "TERM";
    const OFF: &'static str = "0";
    const DUMB: &'static str = "dumb";

    /// `NO_COLOR` turns colors off, then `CLICOLOR_FORCE` on, then `CLICOLOR=0` or
    /// `TERM=dumb` off.
    #[must_use]
    pub fn read() -> Self {
        let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
        let forced = if var(Self::NO_COLOR).is_some() {
            Some(Color::Never)
        } else if var(Self::CLICOLOR_FORCE).is_some_and(|v| v != Self::OFF) {
            Some(Color::Always)
        } else if var(Self::CLICOLOR).is_some_and(|v| v == Self::OFF)
            || var(Self::TERM).is_some_and(|v| v == Self::DUMB)
        {
            Some(Color::Never)
        } else {
            None
        };
        Self(forced)
    }
}

/// A standard stream, to ask whether it is a terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_data::ToData)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    #[must_use]
    pub fn is_terminal(self) -> bool {
        match self {
            Self::Stdout => std::io::stdout().is_terminal(),
            Self::Stderr => std::io::stderr().is_terminal(),
        }
    }

    /// The colors of this stream under `choice` and the process's environment.
    #[must_use]
    pub fn color(self, choice: ColorChoice) -> Color {
        choice.color(Env::read(), self.is_terminal())
    }
}
