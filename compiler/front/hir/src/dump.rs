//! The text form of a HIR for `vun dump hir`: one node per line, indented by depth, with the
//! source position of its origin and, for names, what they resolve to.

use vun_origin::{OriginId, OriginTable};
use vun_outline::{Outline, Outlined};
use vun_span::SourceMap;

use crate::resolve::{Res, Resolution};
use crate::tree::{BlockId, DeclId, ExprId, ExprKind, Hir, Lit, NsId, PatId, PatKind, Spread};
use crate::visit::{self, Visitor};

impl Hir {
    /// Renders this HIR as text, with what its names resolve to.
    #[must_use]
    pub fn dump(&self, resolution: &Resolution, origins: &OriginTable, sources: &dyn SourceMap) -> String {
        let mut printer = Printer { out: Outline::new(), resolution, origins, sources };
        if let Some(root) = self.root {
            printer.namespace(self, root);
        }
        printer.out.finish()
    }
}

struct Printer<'a> {
    out: Outline,
    resolution: &'a Resolution,
    origins: &'a OriginTable,
    sources: &'a dyn SourceMap,
}

impl Outlined for Printer<'_> {
    fn outline(&mut self) -> &mut Outline {
        &mut self.out
    }
}

impl Printer<'_> {
    /// Writes one line for a node, then its children one level deeper.
    fn line(&mut self, origin: OriginId, label: &str, children: impl FnOnce(&mut Self)) {
        let at = self.sources.locate(self.origins.source_span(origin)).at;
        self.node(format_args!("{label} @{}:{}", at.line, at.col), children);
    }

    fn expr_label(&self, hir: &Hir, expr: ExprId) -> String {
        match &hir.exprs[expr].kind {
            ExprKind::Name(s) => {
                let text = s.as_str();
                match self.resolution.names[expr] {
                    Some(Res::Binder(b)) => format!("Name {text} -> binder {}", vun_index::Idx::index(b)),
                    _ => format!("Name {text} -> outer"),
                }
            }
            ExprKind::Lit(Lit::Int(i)) => format!("Int {i}"),
            ExprKind::Lit(Lit::Float(f)) => format!("Float {f}"),
            ExprKind::Lit(Lit::Char(c)) => format!("Char {c:?}"),
            ExprKind::Lit(Lit::String(s)) => format!("String {s:?}"),
            ExprKind::DotName(n) => format!("DotName .{n}"),
            ExprKind::Member { name, .. } => format!("Member .{name}"),
            ExprKind::Prefix { op, .. } | ExprKind::Postfix { op, .. } | ExprKind::Infix { op, .. } => {
                format!("{} {}", hir.exprs[expr].kind.name(), op.text().unwrap_or_default())
            }
            ExprKind::Labeled { label, .. } => format!("Labeled '{}", label.as_str()),
            kind => kind.name().to_owned(),
        }
    }
}

impl Visitor for Printer<'_> {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        self.line(hir.namespaces[ns].origin, "Namespace", |p| visit::walk_namespace(p, hir, ns));
    }

    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        self.line(hir.decls[decl].origin, "Decl", |p| visit::walk_decl(p, hir, decl));
    }

    fn spread(&mut self, hir: &Hir, spread: &Spread) {
        let names: Option<Vec<&str>> =
            spread.select.as_ref().map(|s| s.iter().map(|&b| hir.binders[b].name.as_str()).collect());
        let label = names.map_or_else(|| "Spread".to_owned(), |n| format!("Spread {{ {} }}", n.join(", ")));
        self.line(spread.origin, &label, |p| p.expr(hir, spread.path));
    }

    fn block(&mut self, hir: &Hir, block: BlockId) {
        self.line(hir.blocks[block].origin, "Block", |p| visit::walk_block(p, hir, block));
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        let label = self.expr_label(hir, expr);
        self.line(hir.exprs[expr].origin, &label, |p| visit::walk_expr(p, hir, expr));
    }

    fn pat(&mut self, hir: &Hir, pat: PatId) {
        let label = match &hir.pats[pat].kind {
            PatKind::Bind(b) => {
                let binder = &hir.binders[*b];
                let marks = format!(
                    "{}{}",
                    if binder.mutable { "mut " } else { "" },
                    if binder.comptime { "$" } else { "" }
                );
                format!("Bind {marks}{} = binder {}", binder.name.as_str(), vun_index::Idx::index(*b))
            }
            PatKind::Hole => "Hole".to_owned(),
            PatKind::Tuple(_) => "Tuple".to_owned(),
            PatKind::Variant { name, .. } => format!("Variant .{name}"),
            PatKind::Struct { .. } => "Struct".to_owned(),
            PatKind::Or(_) => "Or".to_owned(),
            PatKind::Range { inclusive, .. } => if *inclusive { "Range ..=" } else { "Range .." }.to_owned(),
            PatKind::Rest(_) => "Rest".to_owned(),
            PatKind::At { binder, .. } => format!("At {}", hir.binders[*binder].name.as_str()),
            PatKind::Value(_) => "Value".to_owned(),
            PatKind::Error => "Error".to_owned(),
        };
        self.line(hir.pats[pat].origin, &format!("pattern {label}"), |p| visit::walk_pat(p, hir, pat));
    }
}
