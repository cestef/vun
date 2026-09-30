//! Views of what sema computes: the type and stage of each expression, the value of each
//! declaration, and the layout of each type.

use std::io::{self, Write};

use vun_hir::db::Parsed;
use vun_hir::tree::ExprKind;
use vun_mir::{Programs, Roots};
use vun_origin::Resolve;
use vun_sema::decl::{DeclType, DeclValue};
use vun_sema::layout::Shapes;
use vun_sema::member::Members;
use vun_sema::typeck::{Operation, Typed};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::DeclRef;
use vun_span::{FileId, SourceMap};

use crate::Session;

/// How much of an expression's source a line of the types shows.
const SHOWN: usize = 48;

impl Session {
    /// What typing found in each function of `file`, plain, and in every body of the program,
    /// from any file, each once: each expression's type and stage, the untyped ones marked,
    /// and each operator without the function it calls.
    pub(super) fn view_types(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let key = self.file_ref(file);
        let program = self.db.get::<Programs>(&(key, Roots::Functions));
        let plain = self.functions(file).into_iter().map(Instantiation::plain);
        let mut seen = std::collections::HashSet::new();
        for i in plain.chain(program.bodies()).filter(|i| seen.insert(i.clone())) {
            let Some(typed) = self.db.get::<Typed>(&i) else { continue };
            let Some(parsed) = self.db.get::<Parsed>(&i.f.literal.file) else { continue };
            let hir = &parsed.hir;
            let f = &i.f.literal;
            writeln!(out, "fn {} {:?}", self.sources.locate(self.source_span(f.expr.origin)), i.args)?;
            let body = hir.exprs.iter().filter(|(e, _)| hir.enclosing_function(*e) == Some(f.expr.id));
            for (e, expr) in body {
                let span = self.source_span(expr.origin);
                let at = self.sources.locate(span).at;
                let source: String =
                    span.range.of(&self.sources.file(span.file).text).chars().take(SHOWN).collect();
                let source = source.replace('\n', " ");
                let known = match typed.types.get(&e) {
                    Some(k) => format!("{} ({})", k.ty, k.stage.name()),
                    None => match typed.gave_up.get(&e) {
                        Some(why) => format!("untyped, gave up on {}: {}", expr.kind.name(), why.msg()),
                        None => "untyped".to_owned(),
                    },
                };
                let operator = matches!(expr.kind, ExprKind::Infix { .. } | ExprKind::Prefix { .. })
                    && !typed.operators.contains_key(&Operation::Expr(e));
                let operator = if operator { ", no operator" } else { "" };
                writeln!(out, "  @{}:{} {source}: {known}{operator}", at.line, at.col)?;
            }
        }
        Ok(())
    }

    /// Each declaration of a namespace of `file`, in source order: where it is, its name, its
    /// type and stage, and its value, or why it has none when compiling.
    pub(super) fn view_values(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let key = self.file_ref(file);
        let parsed = self.parsed(file);
        let hir = &parsed.hir;
        for (id, decl) in hir.decls.iter() {
            let mut binders = Vec::new();
            hir.binders(decl.pat, &mut binders);
            let Some(&binder) = binders.first().filter(|&&b| hir.member_decl(b).is_some()) else { continue };
            let decl = DeclRef::new(key.clone(), hir, id);
            let at = self.sources.locate(self.source_span(hir.pats[hir.decls[id].pat].origin)).at;
            write!(out, "@{}:{} {}", at.line, at.col, hir.binders[binder].name)?;
            if let Ok(known) = self.db.get::<DeclType>(&decl) {
                write!(out, ": {} ({})", known.ty, known.stage.name())?;
            }
            match self.db.get::<DeclValue>(&decl) {
                Ok(value) => writeln!(out, " = {value}")?,
                Err(unevaluated) => writeln!(out, ", no value: {}", unevaluated.why())?,
            }
        }
        Ok(())
    }

    /// The layout of each type `file` binds at its top level.
    pub(super) fn view_layout(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        for (name, ty) in self.types(file) {
            let shape = self.db.get::<Shapes>(&ty);
            let members = self.db.get::<Members>(&ty);
            match shape {
                Some(shape) => writeln!(out, "{name}: {}", shape.text(members.as_deref()))?,
                None => writeln!(out, "{name}: no layout")?,
            }
        }
        Ok(())
    }
}
