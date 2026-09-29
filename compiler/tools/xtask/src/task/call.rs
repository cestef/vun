//! The runs of external programs that tasks make. Each command line is spelled here alone.

use vun_tool::Streams;

use super::Program;

#[derive(Clone, Copy)]
pub(crate) enum Call {
    /// Lists the files rustfmt would change, failing if there are any.
    Formatted,
    Format,
    /// Fails if a generated file is out of date.
    Generated,
    Generate,
    /// `jj git push`, with the words the task is given.
    Push,
}

impl Call {
    pub(crate) const fn program(self) -> Program {
        match self {
            Self::Formatted | Self::Format | Self::Generated | Self::Generate => Program::Cargo,
            Self::Push => Program::Jj,
        }
    }

    /// The words after the program's name.
    pub(crate) const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Formatted => &["fmt", "--check", "--", "--files-with-diff"],
            Self::Format => &["fmt"],
            Self::Generated => &["gen", "--check"],
            Self::Generate => &["gen"],
            Self::Push => &["git", "push"],
        }
    }

    /// The call that repairs what this one finds.
    pub(crate) const fn fix(self) -> Option<Self> {
        match self {
            Self::Formatted => Some(Self::Format),
            Self::Generated => Some(Self::Generate),
            Self::Format | Self::Generate | Self::Push => None,
        }
    }

    /// Where its output goes: a push is watched, and may ask for a passphrase; the output of a
    /// check is read, each line of it a finding.
    pub(crate) const fn streams(self) -> Streams {
        match self {
            Self::Push => Streams::Inherited,
            Self::Formatted | Self::Format | Self::Generated | Self::Generate => Streams::Captured,
        }
    }

    /// The command line, as messages show it.
    pub(crate) fn line(self) -> String {
        std::iter::once(self.program().name())
            .chain(self.words().iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
