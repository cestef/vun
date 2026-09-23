//! Visibility is spelled in the name (spec 01, section 7).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Visible wherever its parent is.
    Public,
    /// Visible only inside the declaring namespace and the namespaces nested in it.
    Private,
}

impl Visibility {
    /// Starts every private name.
    pub const PREFIX: char = '_';

    #[must_use]
    pub fn of(name: &str) -> Self {
        if name.starts_with(Self::PREFIX) { Self::Private } else { Self::Public }
    }
}
