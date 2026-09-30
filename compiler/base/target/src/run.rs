//! The programs a component runs, which building for a target needs on the machine.

use vun_intern::Symbol;

use crate::TargetSpec;

/// A program a backend or a linker runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Run {
    /// The target's C compiler (`lang.target.CCompiler.command`).
    Compiler,
    /// The target's archiver (`lang.target.CCompiler.archiver`).
    Archiver,
    /// A program of this name on the search path.
    Program(&'static str),
}

impl Run {
    /// The command it is for `target`.
    #[must_use]
    pub fn command(self, target: &TargetSpec) -> Symbol {
        match self {
            Self::Compiler => target.c.compiler.command,
            Self::Archiver => target.c.compiler.archiver,
            Self::Program(name) => Symbol::intern(name),
        }
    }
}
