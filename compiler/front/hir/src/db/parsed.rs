//! A file lexed, parsed, lowered and resolved within itself, with what those stages reported.

use std::hash::Hasher;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_desugar::Borrow;
use vun_diag::{DiagCtxt, DiagCx};
use vun_hash::Hash;
use vun_origin::OriginTable;
use vun_query::{Cx, Query};
use vun_span::{FileId, Locations, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;
use vun_trace::tracing;

use super::input::{FileRef, FileText};
use crate::lower;
use crate::resolve::{Res, Resolution};
use crate::tree::{BinderId, ExprId, ExprKind, Hir};

#[derive(Encode, Decode)]
pub struct ParsedFile {
    pub file: FileId,
    pub tokens: Tokens,
    pub cst: Cst,
    pub hir: Hir,
    pub resolution: Resolution,
    /// What the stages reported, returned to whoever reads the file.
    pub diag: DiagCtxt,
    /// The hash of the text, which determines everything else.
    text: Hash,
}

impl ParsedFile {
    /// The binder the capture `capture` of a closure captures (`x`, `&mut x`), and the mode of
    /// the borrow of it the closure holds, if it holds one.
    #[must_use]
    pub fn captured(&self, capture: ExprId) -> Option<(BinderId, Option<Borrow>)> {
        match &self.hir.exprs[capture].kind {
            ExprKind::Borrow { mode, operand } => Some((self.captured(*operand)?.0, Some((*mode)?))),
            ExprKind::Name(_) => match self.resolution.names[capture] {
                Some(Res::Binder(b)) => Some((b, None)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A file is identified by its text: equal texts give equal results.
impl std::hash::Hash for ParsedFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.file.hash(state);
    }
}

/// By the file's key, which names it in every process, as it hashes: one read from saved memos
/// is added under the path this machine shows, which may not be the path the same file was
/// added under meanwhile.
impl PartialEq for ParsedFile {
    fn eq(&self, other: &Self) -> bool {
        (self.text, self.file.key()) == (other.text, other.file.key())
    }
}

impl Eq for ParsedFile {}

/// A parsed file is what it is equal by: its file and the hash of its text.
impl std::fmt::Debug for ParsedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedFile")
            .field("file", &self.file)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// `None` for a file the driver did not read.
pub struct Parsed;

impl Query for Parsed {
    const NAME: &'static str = "parsed";
    type Key = FileRef;
    type Value = Option<Arc<ParsedFile>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        let text = cx.get::<FileText>(key)?;
        let (sources, origins) = (SourceFiles::global(), OriginTable::global());
        let file = sources.add_as(key.clone(), Locations::global().display_in(cx.db().id(), key), &*text);
        let mut diag = DiagCtxt::new();
        let mut dcx = DiagCx { diag: &mut diag, origins };
        let tokens = Tokens::lex(file, &text, &mut dcx);
        let cst = Cst::parse(&tokens, &text, &mut dcx);
        let hir = Hir::lower(lower::Source { cst: &cst, tokens: &tokens, text: &text }, &mut dcx);
        let resolution = hir.resolve(&mut dcx);
        tracing::debug!(path = %key.path, decls = hir.decls.len(), exprs = hir.exprs.len(), "lowered to HIR");
        let text = Hash::of(text.as_bytes());
        Some(Arc::new(ParsedFile { file, tokens, cst, hir, resolution, diag, text }))
    }
}

/// What lexing, parsing, lowering and resolving a file reported, which is saved with its value
/// where the file's own is only traced, so that a run reporting a library file need not read it.
pub struct Reported;

impl Query for Reported {
    const NAME: &'static str = "parse_report";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        Some(Arc::new(cx.get::<Parsed>(key)?.diag.clone()))
    }
}
