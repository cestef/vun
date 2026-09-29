//! How a file-scope declaration is stored: who sees its name, and how long an object lives.

/// Who sees a file-scope name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// `static`: this file only.
    Static,
    /// Declared here, defined elsewhere.
    Extern,
    /// Defined here and seen by the linker.
    Public,
}

/// How long an object lives, as C11 names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Duration {
    /// As long as the program.
    Static,
    /// As long as the thread, each thread having its own (`__thread`, GNU C's).
    Thread,
}
