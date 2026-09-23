//! Compiler diagnostics, mirroring `lang.errors` in `lib/lang`.
//!
//! Each id is declared once here; the explanation pages and `lang.errors` are checked against
//! this list.

use crate::DiagId;

macro_rules! errors {
    ($($name:ident = $path:literal;)*) => {
        $(pub const $name: DiagId = DiagId::new(concat!("lang.errors.", $path));)*

        /// Every compiler diagnostic id.
        pub const ALL: &[DiagId] = &[$($name),*];
    };
}

errors! {
    INT_DOES_NOT_FIT = "int.does_not_fit";
    TYPES_MISMATCH = "types.mismatch";
    BACKEND_UNAVAILABLE = "backend.unavailable";
    LINK_UNAVAILABLE = "link.unavailable";
    CLI_UNKNOWN_COMMAND = "cli.unknown_command";
    CLI_UNAVAILABLE_COMMAND = "cli.unavailable_command";
}

#[cfg(test)]
mod tests {
    use super::ALL;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        assert_eq!(ALL.iter().collect::<HashSet<_>>().len(), ALL.len());
    }
}
