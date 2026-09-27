//! Where origins are in the text a compilation reads now. A result reused after its file was
//! edited, as a saved query is, holds origins recorded in the earlier text. Each is anchored to
//! its declaration, so it is found at the same place relative to that declaration as read in
//! the current text; a declaration is where the current text declares it.

use vun_span::{FileId, Span};

use crate::{Anchor, OriginId, OriginTable};

/// The files one compilation reads, which positions are shown in.
pub trait Resolve {
    fn origins(&self) -> &OriginTable;

    /// The file this compilation reads under the key of `file`: `file` itself unless the file was
    /// read again with another text.
    fn current(&self, file: FileId) -> FileId;

    /// Where `id` is in the files read now; as recorded when its declaration is not in them.
    fn span(&self, id: OriginId) -> Span {
        let origins = self.origins();
        let origin = origins.get(id);
        let now = self.current(origin.span.file);
        if now == origin.span.file {
            return origin.span;
        }
        match origin.anchor {
            Anchor::File => origin.span,
            Anchor::Declaration { .. } => {
                origins.declared(id, now).map_or(origin.span, |d| origins.get(d).span)
            }
            Anchor::Within(declaration) => {
                let then = origins.get(declaration).span;
                let moved = origins.declared(declaration, now).map(|d| origins.get(d).span);
                let offset = origin.span.range.lo.checked_sub(then.range.lo);
                match (moved, offset.filter(|_| then.file == origin.span.file)) {
                    (Some(moved), Some(offset)) => {
                        let lo = moved.range.lo + offset;
                        Span::new(moved.file, lo, lo + origin.span.range.len())
                    }
                    _ => origin.span,
                }
            }
        }
    }

    /// Where the first user-written origin in the chain of `id` is in the files read now.
    fn source_span(&self, id: OriginId) -> Span {
        self.span(self.origins().written(id))
    }
}

/// Positions as recorded, for whoever reads every file in one text.
impl Resolve for OriginTable {
    fn origins(&self) -> &OriginTable {
        self
    }

    fn current(&self, file: FileId) -> FileId {
        file
    }
}

impl OriginTable {
    /// The declaration `id` names, as read in `file`.
    fn declared(&self, id: OriginId, file: FileId) -> Option<OriginId> {
        let index = self.declared.pin().get(&(id.identity, file)).copied()?;
        Some(OriginId { index, identity: id.identity })
    }
}
