//! Reading a lockfile's declarations, reporting each one that does not read at its source.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use vun_diag::{DiagCtxt, DiagCx, Msg};
use vun_origin::OriginTable;
use vun_span::{FileId, SourceFiles, Span};
use vun_syntax::cst::Cst;
use vun_syntax::literal::{Decl, Literal};
use vun_syntax::tokens::Tokens;

use super::{FILE, LAYOUT, Lock, Locked, Pinned, Replaced, Replacing, field};
use crate::manifest::Identity;
use crate::manifest::record::{Fields, Record};

impl Lock {
    /// Reads the lockfile whose declarations are `decls`; `file` locates what is missing.
    pub fn read(decls: &[Decl<'_>], file: FileId, cx: &mut DiagCx<'_>) -> Option<Self> {
        let whole = Decl::new(FILE, Literal::Record(decls.to_vec()), Span::new(file, 0, 0));
        let mut fields =
            Fields::new(&whole, &[field::LAYOUT, field::TOOLCHAIN, field::DEPS, field::REPLACE], cx)?;
        let layout = fields.require(field::LAYOUT).and_then(|d| {
            d.int::<u32>(cx).filter(|&l| l == LAYOUT).or_else(|| {
                d.invalid(Msg::new().text("the layout this toolchain writes, ").code(LAYOUT.to_string()), cx)
            })
        });
        let toolchain = fields.get(field::TOOLCHAIN).map_or(Some(None), |d| Pinned::read(d, cx).map(Some));
        let entries = fields.require(field::DEPS).and_then(|d| d.items(Locked::read, cx));
        let replaced = fields.get(field::REPLACE).map_or(Some(Vec::new()), |d| d.items(Replaced::read, cx));
        fields.finish(cx);
        layout?;
        Some(Self::new(entries?).replacing(replaced?).pinning(toolchain?))
    }

    fn from_text(text: &str) -> Option<Self> {
        let file = SourceFiles::new().add(FILE, text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        let lock = Self::read(&cst.declarations(&tokens, text), file, &mut cx);
        lock.filter(|_| diag.error_count() == 0)
    }
}

/// The text of a lockfile does not read; what does not is reported at its source by [`Lock::read`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable;

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the lockfile does not read")
    }
}

impl Error for Unreadable {}

impl FromStr for Lock {
    type Err = Unreadable;

    fn from_str(text: &str) -> Result<Self, Unreadable> {
        Self::from_text(text).ok_or(Unreadable)
    }
}

impl Locked {
    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::ENTRY, cx)?;
        let name = fields.require(field::NAME).and_then(|d| d.string(cx));
        let version = fields.require(field::VERSION).and_then(|d| d.version(cx));
        let identity = fields.require(field::ID).and_then(|d| match d.value {
            Literal::Name(field::FOREIGN) => Some(None),
            _ => Identity::read(d, cx).map(Some),
        });
        let hash = fields.require(field::HASH).and_then(|d| d.content_hash(cx));
        let via = fields.get(field::VIA).map_or(Some(None), |d| d.string(cx).map(Some));
        fields.finish(cx);
        Some(Self { name: name?, version: version?, identity: identity?, hash: hash?, via: via? })
    }
}

impl Pinned {
    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::PINNED, cx)?;
        let version = fields.require(field::VERSION).and_then(|d| d.version(cx));
        let hash = fields.require(field::HASH).and_then(|d| d.content_hash(cx));
        fields.finish(cx);
        Some(Self { version: version?, hash: hash? })
    }
}

impl Replaced {
    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::REPLACEMENT, cx)?;
        let name = fields.require(field::NAME).and_then(|d| d.string(cx));
        let identity = fields.require(field::ID).and_then(|d| Identity::read(d, cx));
        let with = match (fields.get(field::HASH), fields.get(field::PATH), fields.get(field::LIBRARY)) {
            (Some(hash), None, None) => hash.content_hash(cx).map(Replacing::Stored),
            (None, Some(path), None) => path.string(cx).map(Replacing::Path),
            (None, None, Some(path)) => path.string(cx).map(Replacing::Library),
            _ => decl.invalid(
                Msg::new()
                    .text("a replacement with one of ")
                    .code(format!(".{}", field::HASH))
                    .text(", ")
                    .code(format!(".{}", field::PATH))
                    .text(" and ")
                    .code(format!(".{}", field::LIBRARY)),
                cx,
            ),
        };
        fields.finish(cx);
        Some(Self { name: name?, identity: identity?, with: with? })
    }
}

impl Identity {
    /// An identity written `.{ .id = n, .major = m }`.
    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::IDENTITY, cx)?;
        let id = fields.require(field::ID).and_then(|d| d.int(cx));
        let major = fields.require(field::MAJOR).and_then(|d| d.int(cx));
        fields.finish(cx);
        Some(Self { id: id?, major: major? })
    }
}
