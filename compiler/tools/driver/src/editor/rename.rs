//! Renaming a declaration: its binder and every use, as [`Session::references`] finds them, in the
//! files of the package asking.

use std::fmt;

use vun_sema::site::Declaration;
use vun_sema_ty::item::Site;
use vun_span::{FileId, Span};
use vun_syntax::tokens::Tokens;

use crate::Session;

/// The spans naming a declaration, and the text each becomes.
pub struct Renamed {
    pub text: String,
    pub spans: Vec<Span>,
}

/// Why a declaration is not renamed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unrenamed {
    /// The new name does not lex as one.
    NotAName(String),
    /// Nothing at the position names a declaration.
    Nothing,
    /// A namespace, which its file's name declares.
    Namespace,
    /// Declared in another package, whose files are not the asker's to edit.
    Elsewhere,
}

impl fmt::Display for Unrenamed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAName(name) => write!(f, "`{name}` is not a name"),
            Self::Nothing => f.write_str("nothing here names a declaration"),
            Self::Namespace => f.write_str("a namespace is named by its file"),
            Self::Elsewhere => f.write_str("the declaration is in another package"),
        }
    }
}

impl std::error::Error for Unrenamed {}

impl Session {
    /// What renaming the declaration named at `offset` of `file` to `name` changes.
    ///
    /// # Errors
    /// Returns why it is not renamed.
    pub fn rename(&mut self, file: FileId, offset: u32, name: &str) -> Result<Renamed, Unrenamed> {
        let text = Tokens::spell_name(name).ok_or_else(|| Unrenamed::NotAName(name.to_owned()))?;
        let asking = self.file_ref(file).package;
        let (named, occurrences) = self.occurrences(file, offset).ok_or(Unrenamed::Nothing)?;
        match named.declaration {
            Declaration::Site(Site::Namespace(_)) => return Err(Unrenamed::Namespace),
            Declaration::Site(Site::Decl(decl) | Site::Variant { decl, .. })
                if decl.file.package != asking =>
            {
                return Err(Unrenamed::Elsewhere);
            }
            Declaration::Site(Site::Decl(_) | Site::Variant { .. }) | Declaration::Local(_) => {}
        }
        Ok(Renamed { text, spans: occurrences.all() })
    }
}
