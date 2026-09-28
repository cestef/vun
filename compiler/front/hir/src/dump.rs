//! The text form of a HIR for `vun debug hir`: one node per line, indented by depth, with the
//! source position of its origin and, for names, what they resolve to.

use vun_origin::{OriginId, OriginTable};
use vun_outline::{Outline, Outlined};
use vun_span::SourceMap;

use crate::resolve::{Res, Resolution};
use crate::tree::{
    Asm, AsmEntry, BlockId, DeclId, ExprId, ExprKind, Hir, Lit, NsId, Operand, OperandKind, PatId, PatKind,
    Spread, Writeback,
};
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
            ExprKind::Lit(Lit::String(s)) => format!("String {:?}", String::from_utf8_lossy(s)),
            ExprKind::DotName(n) => format!("DotName .{n}"),
            ExprKind::Member { name, .. } => format!("Member .{name}"),
            ExprKind::Borrow { mode: Some(mode), .. } => format!("Borrow &{mode}"),
            ExprKind::Borrow { mode: None, .. } => "Borrow &?".to_owned(),
            ExprKind::Prefix { op, .. } | ExprKind::Postfix { op, .. } | ExprKind::Infix { op, .. } => {
                format!("{} {}", hir.exprs[expr].kind.name(), op.text().unwrap_or_default())
            }
            ExprKind::Labeled { label, .. } => format!("Labeled '{}", label.as_str()),
            kind => kind.name().to_owned(),
        }
    }
}

impl Printer<'_> {
    /// The architecture of a block, then a line per label and instruction.
    fn asm(&mut self, hir: &Hir, asm: &Asm) {
        self.expr(hir, asm.arch);
        for entry in &asm.entries {
            match entry {
                AsmEntry::Label { name, origin } => self.line(*origin, &format!("Label '{name}"), |_| {}),
                AsmEntry::Inst(inst) => {
                    let mut line = format!("Inst {}", inst.mnemonic);
                    for suffix in &inst.suffixes {
                        line.push('.');
                        line.push_str(suffix.as_str());
                    }
                    self.line(inst.origin, &line, |p| {
                        inst.operands.iter().for_each(|o| p.operand(hir, o));
                    });
                }
            }
        }
    }

    fn operand(&mut self, hir: &Hir, operand: &Operand) {
        let label = match &operand.kind {
            OperandKind::Error => "Error".to_owned(),
            OperandKind::Unfit(_) => "Unfit".to_owned(),
            OperandKind::Word(w) => format!("Word {w}"),
            OperandKind::Int(i) => format!("Int {i}"),
            OperandKind::Label(l) => format!("Label '{l}"),
            OperandKind::Prefix { op, .. } => format!("Prefix {}", op.text().unwrap_or_default()),
            OperandKind::Infix { op, .. } => format!("Infix {}", op.text().unwrap_or_default()),
            OperandKind::Memory { writeback: Writeback::No, .. } => "Memory".to_owned(),
            OperandKind::Memory { writeback: Writeback::Yes, .. } => "Memory !".to_owned(),
            OperandKind::Bound { flow, register, .. } => {
                let flow = flow.and_then(vun_grammar::Flow::text).unwrap_or_default();
                let register = register.map(|r| format!("({r})")).unwrap_or_default();
                format!("Bound {flow}{register}")
            }
        };
        self.line(operand.origin, label.trim_end(), |p| match &operand.kind {
            OperandKind::Prefix { operand, .. } => p.operand(hir, operand),
            OperandKind::Infix { left, right, .. } => {
                p.operand(hir, left);
                p.operand(hir, right);
            }
            OperandKind::Memory { parts, .. } => parts.iter().for_each(|o| p.operand(hir, o)),
            OperandKind::Bound { value: e, .. } | OperandKind::Unfit(e) => p.expr(hir, *e),
            OperandKind::Error | OperandKind::Word(_) | OperandKind::Int(_) | OperandKind::Label(_) => {}
        });
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
        self.line(hir.exprs[expr].origin, &label, |p| match &hir.exprs[expr].kind {
            ExprKind::Asm(asm) => p.asm(hir, asm),
            _ => visit::walk_expr(p, hir, expr),
        });
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
            PatKind::Computed(_) => "Computed".to_owned(),
            PatKind::Error => "Error".to_owned(),
        };
        self.line(hir.pats[pat].origin, &format!("pattern {label}"), |p| visit::walk_pat(p, hir, pat));
    }
}
