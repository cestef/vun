//! The text form of a HIR for `overt dump hir`: one node per line, indented by depth, with the
//! source position of its origin and, for names, what they resolve to.

use overt_intern::Interner;
use overt_origin::{OriginId, OriginTable};
use overt_outline::{Outline, Outlined};
use overt_span::SourceMap;

use crate::resolve::{Res, Resolution};
use crate::tree::{BlockId, DeclId, ExprId, ExprKind, Hir, Lit, Name, NsId, PatId, PatKind, Spread};
use crate::visit::{self, Visitor};

/// Renders `hir` as text.
#[must_use]
pub fn dump(
    hir: &Hir,
    resolution: &Resolution,
    interner: &Interner,
    origins: &OriginTable,
    sources: &dyn SourceMap,
) -> String {
    let mut printer = Printer { out: Outline::new(), resolution, interner, origins, sources };
    if let Some(root) = hir.root {
        printer.namespace(hir, root);
    }
    printer.out.finish()
}

struct Printer<'a> {
    out: Outline,
    resolution: &'a Resolution,
    interner: &'a Interner,
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

    fn name(&self, name: Name) -> String {
        match name {
            Name::Plain(s) => self.interner.resolve(s).to_owned(),
            Name::Computed(_) => "$(...)".to_owned(),
            Name::Index(i) => i.to_string(),
            Name::Hole => "_".to_owned(),
        }
    }

    fn expr_label(&self, hir: &Hir, expr: ExprId) -> String {
        match &hir.exprs[expr].kind {
            ExprKind::Name(s) => {
                let text = self.interner.resolve(*s);
                match self.resolution.names[expr] {
                    Some(Res::Binder(b)) => format!("Name {text} -> binder {}", overt_index::Idx::index(b)),
                    _ => format!("Name {text} -> outer"),
                }
            }
            ExprKind::Lit(Lit::Int(i)) => format!("Int {i}"),
            ExprKind::Lit(Lit::Float(f)) => format!("Float {f}"),
            ExprKind::Lit(Lit::Char(c)) => format!("Char {c:?}"),
            ExprKind::Lit(Lit::String(s)) => format!("String {s:?}"),
            ExprKind::DotName(n) => format!("DotName .{}", self.name(*n)),
            ExprKind::Member { name, .. } => format!("Member .{}", self.name(*name)),
            ExprKind::Prefix { op, .. } | ExprKind::Postfix { op, .. } | ExprKind::Infix { op, .. } => {
                format!("{} {}", variant(&hir.exprs[expr].kind), op.text().unwrap_or_default())
            }
            ExprKind::Labeled { label, .. } => format!("Labeled '{}", self.interner.resolve(*label)),
            kind => variant(kind).to_owned(),
        }
    }
}

/// The name of an expression kind, as the dump prints it.
fn variant(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::Error => "Error",
        ExprKind::Hole => "Hole",
        ExprKind::Name(_) => "Name",
        ExprKind::Lit(_) => "Lit",
        ExprKind::DotName(_) => "DotName",
        ExprKind::Member { .. } => "Member",
        ExprKind::Call { .. } => "Call",
        ExprKind::Index { .. } => "Index",
        ExprKind::StructLit { .. } => "StructLit",
        ExprKind::Tuple(_) => "Tuple",
        ExprKind::Array(_) => "Array",
        ExprKind::Repeat { .. } => "Repeat",
        ExprKind::ArrayType { .. } => "ArrayType",
        ExprKind::Prefix { .. } => "Prefix",
        ExprKind::Postfix { .. } => "Postfix",
        ExprKind::Infix { .. } => "Infix",
        ExprKind::Block(_) => "Block",
        ExprKind::Quote(_) => "Quote",
        ExprKind::If { .. } => "If",
        ExprKind::For { .. } => "For",
        ExprKind::Match { .. } => "Match",
        ExprKind::Labeled { .. } => "Labeled",
        ExprKind::Return(_) => "Return",
        ExprKind::Break { .. } => "Break",
        ExprKind::Continue { .. } => "Continue",
        ExprKind::Yield(_) => "Yield",
        ExprKind::Defer(_) => "Defer",
        ExprKind::Fn { .. } => "Fn",
        ExprKind::FnType { .. } => "FnType",
        ExprKind::Closure { .. } => "Closure",
        ExprKind::Struct(_) => "Struct",
        ExprKind::Enum(_) => "Enum",
        ExprKind::Attributed { .. } => "Attributed",
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
        let names: Option<Vec<&str>> = spread
            .select
            .as_ref()
            .map(|s| s.iter().map(|&b| self.interner.resolve(hir.binders[b].name)).collect());
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
                format!(
                    "Bind {marks}{} = binder {}",
                    self.interner.resolve(binder.name),
                    overt_index::Idx::index(*b)
                )
            }
            PatKind::Hole => "Hole".to_owned(),
            PatKind::Tuple(_) => "Tuple".to_owned(),
            PatKind::Variant { name, .. } => format!("Variant .{}", self.name(*name)),
            PatKind::Struct { .. } => "Struct".to_owned(),
            PatKind::Or(_) => "Or".to_owned(),
            PatKind::Range { inclusive, .. } => if *inclusive { "Range ..=" } else { "Range .." }.to_owned(),
            PatKind::Rest(_) => "Rest".to_owned(),
            PatKind::At { binder, .. } => format!("At {}", self.interner.resolve(hir.binders[*binder].name)),
            PatKind::Value(_) => "Value".to_owned(),
            PatKind::Error => "Error".to_owned(),
        };
        self.line(hir.pats[pat].origin, &format!("pattern {label}"), |p| visit::walk_pat(p, hir, pat));
    }
}
