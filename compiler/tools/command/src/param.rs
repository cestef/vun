//! Parameters as data: what a command takes, how often, what values it accepts and where a
//! default comes from. Help, parsing, validation and completions all read the same declaration.

/// A parameter, declared as a constant with the builders below:
/// `Param::option("target").about("...").values(Values::Text).env("VUN_TARGET")`.
#[derive(Debug)]
pub struct Param {
    pub name: &'static str,
    pub short: Option<char>,
    pub about: &'static str,
    pub kind: ParamKind,
    pub values: Values,
    /// The environment variable giving its value when the command line does not.
    pub env: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// A switch: `--name`.
    Flag,
    /// A named value: `--name <value>` or `--name=value`.
    Option(Arity),
    /// A value by position, in declaration order: `<name>`. Words after `--` are positional.
    Positional(Arity),
}

/// How many values a parameter takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arity {
    /// Exactly one for a positional; at most one for an option.
    One,
    /// At most one.
    Optional,
    /// Any number, a positional taking every word left.
    Many,
}

/// What a value may be: shown in help, checked while parsing, offered by completions.
#[derive(Clone, Copy, Debug)]
pub enum Values {
    Text,
    Path(PathKind),
    /// One of the names a table gives, such as the diagnostic formats or the backends.
    Choice(fn() -> Vec<&'static str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    File,
    Dir,
}

impl Param {
    const fn new(name: &'static str, kind: ParamKind, values: Values) -> Self {
        Self { name, short: None, about: "", kind, values, env: None }
    }

    #[must_use]
    pub const fn flag(name: &'static str) -> Self {
        Self::new(name, ParamKind::Flag, Values::Text)
    }

    #[must_use]
    pub const fn option(name: &'static str) -> Self {
        Self::new(name, ParamKind::Option(Arity::One), Values::Text)
    }

    #[must_use]
    pub const fn positional(name: &'static str) -> Self {
        Self::new(name, ParamKind::Positional(Arity::One), Values::Text)
    }

    #[must_use]
    pub const fn short(self, short: char) -> Self {
        Self { short: Some(short), ..self }
    }

    #[must_use]
    pub const fn about(self, about: &'static str) -> Self {
        Self { about, ..self }
    }

    #[must_use]
    pub const fn values(self, values: Values) -> Self {
        Self { values, ..self }
    }

    /// Reads its value from `env` when the command line gives none.
    ///
    /// # Panics
    /// When declared on a flag, which has no value to read: at compile time, as parameters are
    /// constants.
    #[must_use]
    pub const fn env(self, env: &'static str) -> Self {
        assert!(!matches!(self.kind, ParamKind::Flag), "a flag takes no value from a variable");
        Self { env: Some(env), ..self }
    }

    /// Takes `arity` values rather than one.
    #[must_use]
    pub const fn arity(self, arity: Arity) -> Self {
        let kind = match self.kind {
            ParamKind::Flag => ParamKind::Flag,
            ParamKind::Option(_) => ParamKind::Option(arity),
            ParamKind::Positional(_) => ParamKind::Positional(arity),
        };
        Self { kind, ..self }
    }

    /// Whether this is `other`: parameters are named uniquely in a command, but a command's own
    /// may share a name with a global one of another kind, as `pkg add --version <v>` does with
    /// the root's `--version` flag.
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        self.name == other.name && self.kind == other.kind && self.short == other.short
    }

    #[must_use]
    pub const fn is_positional(&self) -> bool {
        matches!(self.kind, ParamKind::Positional(_))
    }

    /// Whether a run needs a value for it.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        matches!(self.kind, ParamKind::Positional(Arity::One))
    }

    #[must_use]
    pub const fn takes_value(&self) -> bool {
        matches!(self.kind, ParamKind::Option(_))
    }

    /// The names its value may take, when it is a choice.
    #[must_use]
    pub fn choices(&self) -> Option<Vec<&'static str>> {
        match self.values {
            Values::Choice(names) => Some(names()),
            Values::Text | Values::Path(_) => None,
        }
    }
}

pub const HELP: Param = Param::flag("help").short('h').about("show help; `--help` shows more");
pub const VERSION: Param = Param::flag("version").short('V').about("show the version");
pub const COLOR: Param = Param::option("color")
    .about("when to show colors")
    .values(Values::Choice(|| vun_term::ColorChoice::ALL.iter().map(|c| c.name()).collect()))
    .env("VUN_COLOR");
pub const FORMAT: Param = Param::option("format")
    .about("how output is shown: for people, or JSON lines for programs")
    .values(Values::Choice(|| vun_term::Format::ALL.iter().map(|f| f.name()).collect()))
    .env("VUN_FORMAT");
pub const QUIET: Param =
    Param::flag("quiet").short('q').about("show results and diagnostics, without status lines");
/// Options every command accepts, applied by the program before the command runs; the root
/// accepts `VERSION` besides.
pub const GLOBAL: OptionSet = OptionSet { title: "Global options", params: &[COLOR, FORMAT, QUIET, HELP] };

/// Options several commands take, shown in help under their own title, as cargo's
/// `Compilation Options`.
#[derive(Debug)]
pub struct OptionSet {
    pub title: &'static str,
    pub params: &'static [Param],
}

/// The values given to a command's parameters.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub(crate) values: Vec<(&'static str, String)>,
    pub(crate) flags: Vec<&'static str>,
}

impl Args {
    /// The value of a parameter taking one; a required positional is always present when a
    /// command runs.
    #[must_use]
    pub fn value(&self, param: &Param) -> Option<&str> {
        self.values(param).next()
    }

    /// Every value given to `param`, in order.
    pub fn values(&self, param: &Param) -> impl Iterator<Item = &str> {
        let name = param.name;
        self.values.iter().filter(move |(given, _)| *given == name).map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub fn flag(&self, param: &Param) -> bool {
        self.flags.contains(&param.name)
    }

    /// The entry of `table` a choice parameter names; parsing checked that one does.
    pub fn choice<'t, T>(&self, param: &Param, table: &'t [T], name: impl Fn(&T) -> &str) -> Option<&'t T> {
        let value = self.value(param)?;
        table.iter().find(|t| name(t) == value)
    }
}
