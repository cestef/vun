//! Where a declaration is named: its binder, and every name, member and path resolving to it. A
//! local is named in its file alone; a declaration in the files of its package and of the package
//! asking.

use std::collections::HashMap;
use std::sync::Arc;

use vun_grammar::TokenKind;
use vun_hir::db::{FileRef, Package, Parsed, ParsedFile};
use vun_hir::tree::{BinderId, ExprKind, Name};
use vun_intern::Symbol;
use vun_origin::Resolve;
use vun_par::Parallel;
use vun_sema::site::{Declaration, NameSite};
use vun_sema_ty::item::{DeclRef, ExprRef, Site};
use vun_span::{FileId, Span};

use crate::Session;
use crate::positions::Reader;

/// A declaration and the name it is named by: one pattern may declare several names.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Named {
    pub(super) declaration: Declaration,
    pub(super) name: Symbol,
}

/// Where a declaration is named, each span its name alone.
pub struct Occurrences {
    /// Its binder, where the files searched hold it.
    pub declaration: Option<Span>,
    pub uses: Vec<Span>,
}

impl Occurrences {
    /// Every span, the declaration's first.
    #[must_use]
    pub fn all(&self) -> Vec<Span> {
        self.declaration.iter().chain(&self.uses).copied().collect()
    }
}

impl Session {
    /// Where the declaration named at `offset` of `file` is named, by a use or its binder.
    pub fn references(&mut self, file: FileId, offset: u32) -> Option<Occurrences> {
        self.occurrences(file, offset).map(|(_, occurrences)| occurrences)
    }

    /// What is named at `offset` of `file`, and where.
    pub(super) fn occurrences(&mut self, file: FileId, offset: u32) -> Option<(Named, Occurrences)> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key)?;
        let named = self.named_at(&key, &parsed, file, offset)?;
        let files = match &named.declaration {
            Declaration::Local(_) => vec![key],
            Declaration::Site(site) => self.searched(&key, site),
        };
        let reader = self.reader();
        let mut occurrences = Occurrences { declaration: None, uses: Vec::new() };
        for (declaration, uses) in files.par_map(|file| Self::named_in(reader, file, &named)) {
            occurrences.declaration = occurrences.declaration.or(declaration);
            occurrences.uses.extend(uses);
        }
        occurrences.uses.sort_by_key(|s| (s.file, s.range.lo));
        Some((named, occurrences))
    }

    /// Where `file` names `named`: its binder, if there, and its uses.
    fn named_in(reader: Reader<'_>, file: &FileRef, named: &Named) -> (Option<Span>, Vec<Span>) {
        let Some(parsed) = reader.db.get::<Parsed>(file) else { return (None, Vec::new()) };
        let tokens = NameTokens::of(&parsed);
        let hir = &parsed.hir;
        let declaration = hir
            .binders
            .iter()
            .find(|&(b, binder)| {
                binder.name == named.name && Self::declares(file, &parsed, b) == named.declaration
            })
            .and_then(|(_, binder)| tokens.at(reader.source_span(binder.origin)));
        let uses = hir
            .exprs
            .iter()
            .filter(|(_, expr)| match &expr.kind {
                ExprKind::Name(name) | ExprKind::Member { name: Name::Plain(name), .. } => {
                    *name == named.name
                }
                _ => false,
            })
            .filter(|&(id, _)| {
                reader.db.get::<NameSite>(&ExprRef::new(file.clone(), hir, id)).as_ref()
                    == Some(&named.declaration)
            })
            .filter_map(|(_, expr)| tokens.at(reader.source_span(expr.origin)))
            .collect();
        (declaration, uses)
    }

    /// What is named at `offset` of `file`: the binder whose name holds it, or the innermost name
    /// or member holding it.
    fn named_at(&mut self, key: &FileRef, parsed: &ParsedFile, file: FileId, offset: u32) -> Option<Named> {
        let holds = |span: Span| span.file == file && span.range.lo <= offset && offset <= span.range.hi;
        let binder = parsed.hir.binders.iter().find(|(_, b)| holds(self.source_span(b.origin)));
        if let Some((b, binder)) = binder {
            return Some(Named { declaration: Self::declares(key, parsed, b), name: binder.name });
        }
        let expr = self.expr_at(file, offset)?;
        let name = match &parsed.hir.exprs[expr.expr.id].kind {
            ExprKind::Name(name) | ExprKind::Member { name: Name::Plain(name), .. } => *name,
            _ => return None,
        };
        Some(Named { declaration: self.db.get::<NameSite>(&expr)?, name })
    }

    /// What the binder `b` of `file` declares, as a name resolving to it leads there.
    fn declares(file: &FileRef, parsed: &ParsedFile, b: BinderId) -> Declaration {
        let hir = &parsed.hir;
        match hir.member_decl(b) {
            Some(decl) => Declaration::Site(Site::Decl(DeclRef::new(file.clone(), hir, decl))),
            None => Declaration::Local(hir.binders[b].origin),
        }
    }

    /// The files a declaration at `site` may be named in: `asking`, and those of its package and
    /// of the package of `asking`.
    fn searched(&mut self, asking: &FileRef, site: &Site) -> Vec<FileRef> {
        let declaring = match site {
            Site::Decl(decl) => decl.file.package,
            Site::Namespace(ns) => ns.package,
        };
        let mut packages = vec![asking.package, declaring];
        packages.dedup();
        // A loose file lists no files of its own.
        let mut files = vec![asking.clone()];
        for package in packages {
            let data = self.db.get::<Package>(&package);
            files.extend(data.files.iter().map(|path| FileRef { package, path: Symbol::intern(path) }));
        }
        files.sort();
        files.dedup();
        files
    }
}

/// The name tokens of a file, by where each ends: a name is the last token of the expression or
/// binder naming it (`a.b` ends with `b`, `$n` with `n`).
struct NameTokens<'p> {
    parsed: &'p Arc<ParsedFile>,
    ending: HashMap<u32, vun_syntax::tokens::TokenId>,
}

impl<'p> NameTokens<'p> {
    fn of(parsed: &'p Arc<ParsedFile>) -> Self {
        let tokens = &parsed.tokens;
        let ending = tokens
            .ids()
            .filter(|&t| matches!(tokens.kind(t), TokenKind::Ident | TokenKind::RawIdent))
            .map(|t| (tokens.range(t).hi, t))
            .collect();
        Self { parsed, ending }
    }

    /// The name ending where `span` does.
    fn at(&self, span: Span) -> Option<Span> {
        let &t = self.ending.get(&span.range.hi)?;
        let range = self.parsed.tokens.range(t);
        Some(Span::new(span.file, range.lo, range.hi))
    }
}
