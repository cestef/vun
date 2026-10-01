//! What a project says of the options of commands (spec 12, section 2): the values it gives them
//! where neither the command line nor the environment does.

/// A value a project gives an option of a command: `.fmt = .{ .width = 100 }` is the command `fmt`,
/// its option `width`, and the value `100`. An option taking several values has several.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    pub command: String,
    pub option: String,
    pub values: Vec<String>,
}

/// What a project gives the options of commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings(Vec<Setting>);

impl Settings {
    #[must_use]
    pub fn new(settings: Vec<Setting>) -> Self {
        Self(settings)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Setting> {
        self.0.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What it gives the options of `command`.
    pub fn of<'s>(&'s self, command: &'s str) -> impl Iterator<Item = &'s Setting> {
        self.0.iter().filter(move |s| s.command == command)
    }
}
