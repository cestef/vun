//! What a call's callee takes, shown while its arguments are written: the callee's declaration as
//! its source writes it, its parameters, the one the argument at the position fills, and its doc
//! comment.

use vun_hir::db::Parsed;
use vun_hir::tree::{Arg, Decl, ExprId, ExprKind, Hir, Name, each_entry};
use vun_origin::{OriginId, Resolve};
use vun_sema::site::{Declaration, NameSite};
use vun_sema_ty::item::{ExprRef, Site};
use vun_span::{FileId, SourceMap, Span};

use crate::Session;

pub struct Signature {
    /// The callee's name and parameters, as written: `norm(p: Point) -> u8`.
    pub label: String,
    /// Each parameter, as written.
    pub params: Vec<String>,
    /// The parameter the argument at the position fills.
    pub active: usize,
    pub doc: Vec<String>,
}

impl Session {
    /// The signature of the innermost call of `file` whose arguments hold `offset`.
    pub fn signature(&mut self, file: FileId, offset: u32) -> Option<Signature> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key)?;
        let hir = &parsed.hir;
        let span = |id: ExprId| self.source_span(hir.exprs[id].origin);
        let (_, callee, args) = hir
            .exprs
            .iter()
            .filter_map(|(id, e)| match &e.kind {
                ExprKind::Call { callee, args } => Some((span(id), *callee, args)),
                _ => None,
            })
            .filter(|(call, callee, _)| {
                call.file == file && span(*callee).range.hi < offset && offset < call.range.hi
            })
            .min_by_key(|(call, ..)| call.range.len())?;
        let mut active = 0;
        each_entry(args, &mut |a: &Arg| active += usize::from(span(a.value).range.hi < offset));
        let (ExprKind::Name(name) | ExprKind::Member { name: Name::Plain(name), .. }) =
            hir.exprs[callee].kind
        else {
            return None;
        };
        let (declaring, decl) = match self.db.get::<NameSite>(&ExprRef::new(key.clone(), hir, callee))? {
            Declaration::Site(Site::Decl(decl)) => (decl.file, decl.decl.id),
            Declaration::Local(origin) => (key, Self::local_decl(hir, origin)?),
            Declaration::Site(Site::Namespace(_) | Site::Variant { .. }) => return None,
        };
        let declared = self.db.get::<Parsed>(&declaring)?;
        let decl = &declared.hir.decls[decl];
        let (written, params) = self.written_function(&declared.hir, decl)?;
        let text = &self.sources.file(written.file).text;
        let slice = |s: Span| text[s.range.lo as usize..s.range.hi as usize].to_owned();
        let label = [name.as_str(), slice(written).trim_end()].concat();
        Some(Signature {
            label,
            params: params.into_iter().map(slice).collect(),
            active,
            doc: decl.doc.clone(),
        })
    }

    /// The declaration of the local binder recorded at `origin`.
    fn local_decl(hir: &Hir, origin: OriginId) -> Option<vun_hir::tree::DeclId> {
        let (b, _) = hir.binders.iter().find(|(_, b)| b.origin == origin)?;
        hir.decl_of(b)
    }

    /// Where `decl` writes its function's parameters and result, up to its body, and each
    /// parameter: from its value, a function literal, or else from its type, a function type.
    fn written_function(&self, hir: &Hir, decl: &Decl) -> Option<(Span, Vec<Span>)> {
        let mut at = decl.value.or(decl.ty)?;
        loop {
            let (params, end) = match &hir.exprs[at].kind {
                ExprKind::Attributed { inner, .. } | ExprKind::Closure { func: inner, .. } => {
                    at = *inner;
                    continue;
                }
                ExprKind::Fn { params, body, .. } => {
                    (params, Some(self.source_span(hir.blocks[*body].origin)))
                }
                ExprKind::FnType { params, .. } => (params, None),
                _ if Some(at) == decl.value => {
                    at = decl.ty?;
                    continue;
                }
                _ => return None,
            };
            let whole = self.source_span(hir.exprs[at].origin);
            let hi = end.map_or(whole.range.hi, |body| body.range.lo);
            let mut spans = Vec::new();
            each_entry(params, &mut |p| spans.push(self.source_span(p.origin)));
            return Some((Span::new(whole.file, whole.range.lo, hi), spans));
        }
    }
}
