use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;

use vun_diag::{DiagCtxt, DiagCx};
use vun_fs::Edit;
use vun_origin::OriginTable;
use vun_span::{SourceFiles, Span};
use vun_syntax::cst::Cst;
use vun_syntax::literal::{Decl, Fields, FromLiteral, Literal, Mismatch, Unwritable};
use vun_syntax::tokens::Tokens;

use super::Counts;

/// What the baseline file holds.
#[derive(FromLiteral)]
struct Held {
    counts: Fields<u32>,
}

/// Why the baseline is not read or written.
#[derive(Debug)]
pub(crate) enum BaselineError {
    Io(io::Error),
    /// The file is not Vun source; the lexer and parser reported where.
    Syntax,
    Mismatch(Mismatch),
    Unwritable(Unwritable),
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "the baseline cannot be read or written",
            Self::Syntax => "the baseline is not Vun source",
            Self::Mismatch(_) => "the baseline does not hold the counts of the groups",
            Self::Unwritable(_) => "the counts have no Vun source",
        })
    }
}

impl Error for BaselineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Mismatch(source) => Some(source),
            Self::Unwritable(source) => Some(source),
            Self::Syntax => None,
        }
    }
}

impl Counts {
    /// Where the baseline is, relative to the repository root.
    pub(crate) const BASELINE: &str = "compiler/tools/xtask/comments.vun";
    const FIELD: &str = "counts";

    /// The baseline under `root`.
    ///
    /// # Errors
    /// Returns why the file is not read or does not hold counts.
    pub(crate) fn baseline(root: &Path) -> Result<Self, BaselineError> {
        let text = std::fs::read_to_string(root.join(Self::BASELINE)).map_err(BaselineError::Io)?;
        Self::from_source(&text)
    }

    pub(crate) fn from_source(text: &str) -> Result<Self, BaselineError> {
        let file = SourceFiles::new().add(Self::BASELINE, text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        if diag.error_count() > 0 {
            return Err(BaselineError::Syntax);
        }
        let held = Held::from_decls(&cst.declarations(&tokens, text)).map_err(BaselineError::Mismatch)?;
        Ok(Self(held.counts.0.into_iter().collect()))
    }

    /// The Vun source of the counts.
    ///
    /// # Errors
    /// Returns why a group name has no source.
    pub(crate) fn source(&self) -> Result<String, BaselineError> {
        let span = Span::new(SourceFiles::new().add(Self::BASELINE, String::new()), 0, 0);
        let groups =
            self.0.iter().map(|(group, count)| Decl::new(group, Literal::Int(i128::from(*count)), span));
        let whole = Decl::new(Self::FIELD, Literal::Record(groups.collect()), span);
        whole.listed().map_err(BaselineError::Unwritable)
    }

    /// Writes these counts as the baseline under `root`.
    ///
    /// # Errors
    /// Returns why the counts are not written.
    pub(crate) fn bless(&self, root: &Path) -> Result<(), BaselineError> {
        root.join(Self::BASELINE).put(self.source()?).map_err(BaselineError::Io)
    }
}
