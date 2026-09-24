//! How much of a program the stages that exist understand: the share of function-body
//! expressions sema types and of bodies MIR lowers, the progress measure of roadmap phase 1.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::AddAssign;

use vun_hir::db::{FileRef, Parsed};
use vun_hir::tree::ExprKind;
use vun_mir::{Mir, Unbuilt};
use vun_sema::eval::Instantiation;
use vun_sema::typeck::Typed;
use vun_span::FileId;

use crate::Session;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub bodies: usize,
    pub lowered: usize,
    pub exprs: usize,
    pub typed: usize,
    /// The bodies MIR does not lower, by the kind of the form that stopped it.
    pub unbuilt: BTreeMap<&'static str, usize>,
}

/// How many kinds of unbuilt forms the summary names.
const UNBUILT_SHOWN: usize = 6;

impl Session {
    /// The coverage of the functions of `file`.
    pub fn coverage(&mut self, file: FileId) -> Coverage {
        let mut coverage = Coverage::default();
        for f in self.functions(file) {
            coverage.bodies += 1;
            let file = f.file.clone();
            let f = Instantiation::plain(f);
            match self.db.get::<Mir>(&f) {
                Ok(_) => coverage.lowered += 1,
                Err(unbuilt) => *coverage.unbuilt.entry(self.form_at(&file, unbuilt)).or_default() += 1,
            }
            // A closure's body is typed, and counted, with the body it is written in.
            let closure =
                self.db.get::<Parsed>(&file).is_some_and(|p| p.hir.closure_of(f.f.literal.expr.id).is_some());
            if !closure && let Some(typed) = self.db.get::<Typed>(&f) {
                coverage.exprs += typed.exprs;
                coverage.typed += typed.typed_count();
            }
        }
        coverage
    }
}

impl Session {
    /// The kind of the form MIR stopped at: an operator by its text, another expression by its
    /// kind, or what else it is.
    fn form_at(&self, file: &FileRef, unbuilt: Unbuilt) -> &'static str {
        let Some(parsed) = self.db.get::<Parsed>(file) else { return "unread" };
        let hir = &parsed.hir;
        if let Some((_, e)) = hir.exprs.iter().find(|(_, e)| e.origin == unbuilt.at) {
            return match &e.kind {
                ExprKind::Prefix { op, .. } | ExprKind::Postfix { op, .. } | ExprKind::Infix { op, .. } => {
                    op.text().unwrap_or(e.kind.name())
                }
                kind => kind.name(),
            };
        }
        if hir.decls.iter().any(|(_, d)| d.origin == unbuilt.at) {
            return "declaration";
        }
        "statement"
    }
}

impl AddAssign for Coverage {
    fn add_assign(&mut self, other: Self) {
        self.bodies += other.bodies;
        self.lowered += other.lowered;
        self.exprs += other.exprs;
        self.typed += other.typed;
        for (form, n) in other.unbuilt {
            *self.unbuilt.entry(form).or_default() += n;
        }
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percent = |part: usize, whole: usize| (part * 100).checked_div(whole).unwrap_or(0);
        write!(
            f,
            "sema types {} of {} function-body expressions ({}%); MIR lowers {} of {} functions ({}%)",
            self.typed,
            self.exprs,
            percent(self.typed, self.exprs),
            self.lowered,
            self.bodies,
            percent(self.lowered, self.bodies),
        )?;
        let mut unbuilt: Vec<_> = self.unbuilt.iter().collect();
        unbuilt.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (i, (form, n)) in unbuilt.into_iter().take(UNBUILT_SHOWN).enumerate() {
            write!(f, "{}{form} {n}", if i == 0 { "; stopped at " } else { ", " })?;
        }
        Ok(())
    }
}
