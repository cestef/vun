//! A file lexed, parsed, lowered and resolved within itself, with what those stages reported.

use std::hash::Hasher;
use std::sync::Arc;

use overt_diag::{DiagCtxt, DiagCx};
use overt_hash::Hash;
use overt_origin::OriginTable;
use overt_query::{Cx, Query};
use overt_span::{FileId, SourceFiles};
use overt_syntax::cst::Cst;
use overt_syntax::tokens::Tokens;

use super::input::{FileRef, FileText, Package};
use crate::lower;
use crate::resolve::Resolution;
use crate::tree::Hir;

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

/// A file is identified by its text: equal texts give equal results.
impl std::hash::Hash for ParsedFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.file.hash(state);
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
        let package = cx.get::<Package>(&key.package);
        let (sources, origins) = (cx.db().resource::<SourceFiles>(), cx.db().resource::<OriginTable>());
        let file = sources.add_as(&package.file_key(&key.path), package.display(&key.path), &*text);
        let mut diag = DiagCtxt::new();
        let mut dcx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, &text, &mut dcx);
        let cst = Cst::parse(&tokens, &text, &mut dcx);
        let hir = Hir::lower(lower::Source { cst: &cst, tokens: &tokens, text: &text }, &origins);
        let resolution = hir.resolve(&mut dcx);
        let text = Hash::of(text.as_bytes());
        Some(Arc::new(ParsedFile { file, tokens, cst, hir, resolution, diag, text }))
    }
}
