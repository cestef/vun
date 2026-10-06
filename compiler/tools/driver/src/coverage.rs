//! How much of a program the stages that exist understand: the share of function-body
//! expressions sema types, of function literals whose run-time instantiations MIR builds, and of
//! the functions reached that lower to LIR, the progress measure of roadmap phase 1. Functions
//! are reached from every function of the file that depends on no compile-time binding
//! (`Roots::Functions`); a generic one counts by the instantiations reached.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::AddAssign;

use vun_diag::Diagnostic;
use vun_hir::db::{FileRef, Parsed};
use vun_hir::tree::ExprKind;
use vun_lower::{EachLowered, Unlowered, Why};
use vun_mir::{Mir, Programs, Roots, Unbuilt, Unready};
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::eval::Instantiate;
use vun_sema::typed::Typed;
use vun_sema_ty::call::Instantiation;
use vun_span::FileId;

use crate::Session;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub exprs: usize,
    pub typed: usize,
    /// The function literals that run at run time, and those whose every instantiation reached
    /// MIR builds; the others run only at compile time.
    pub bodies: usize,
    pub compile_time: usize,
    pub built: usize,
    /// The functions reached at run time, and those that lower to LIR.
    pub reached: usize,
    pub lowered: usize,
    /// What stops each literal or function short.
    pub gaps: Vec<Gap>,
}

/// A function literal or a function reached that a stage stops at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    /// The function literal.
    pub function: OriginId,
    pub kind: GapKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GapKind {
    /// No function of the file reaches the literal at run time and no compile-time code runs it:
    /// it is generic and never instantiated, or sits in a body that stops short.
    Unreached,
    /// MIR does not build an instantiation of it, stopped at a form of this kind.
    Unready { unready: Unready, form: &'static str },
    /// It does not lower to LIR.
    Unlowered(Unlowered),
}

/// How many kinds of gaps the summary names per stage.
const GAPS_SHOWN: usize = 6;

impl Session {
    /// The coverage of the functions of `file`.
    pub fn coverage(&mut self, file: FileId) -> Coverage {
        let mut coverage = Coverage::default();
        let key = self.file_ref(file);
        let program = self.db.get::<Programs>(&(key.clone(), Roots::Functions));
        // What the checks ran at compile time: calls in declarations' values, `$` code.
        let ran: Vec<(Instantiation, Result<_, Unbuilt>)> = self
            .db
            .computed::<Mir>()
            .into_iter()
            .filter(|(i, _)| i.f.literal.file == key && !i.is_plain())
            .collect();
        for f in self.functions(file) {
            let origin = f.expr.origin;
            let plain = Instantiation::plain(f.clone());
            let parsed = self.db.get::<Parsed>(&key);
            let closure = parsed.as_ref().is_some_and(|p| p.hir.closure_of(f.expr.id).is_some());
            // A closure's body is typed, and counted, with the body it is written in.
            if !closure && let Some(typed) = self.db.get::<Typed>(&plain) {
                coverage.exprs += typed.exprs;
                coverage.typed += typed.typed_count();
            }
            let instantiations: Vec<&Instantiation> =
                program.instantiations().filter(|i| i.f.literal == f).collect();
            // A closure outside any function is in a declaration's value, which only compile-time
            // code evaluates: it runs only then, unless the program reaches it.
            let declared = closure
                && instantiations.is_empty()
                && parsed.as_ref().is_some_and(|p| p.hir.enclosing_function(f.expr.id).is_none());
            if declared || plain.compile_time(&Cx::new(&self.db)) {
                coverage.compile_time += 1;
                continue;
            }
            coverage.bodies += 1;
            let unready = program.unready.iter().find(|(i, _)| i.f.literal == f).map(|(_, u)| u.clone());
            // A literal only compile-time code calls is covered by the bodies built for it.
            let at_compile_time = ran.iter().filter(|(i, _)| i.f.literal == f).map(|(_, built)| built);
            let unbuilt = at_compile_time.clone().find_map(|built| built.as_ref().err().cloned());
            let kind = match (unready, unbuilt) {
                (_, Some(unbuilt)) if instantiations.is_empty() => {
                    let form = self.form_at(&key, unbuilt.at);
                    Some(GapKind::Unready { unready: Unready::Unbuilt(unbuilt), form })
                }
                _ if instantiations.is_empty() && at_compile_time.count() == 0 => Some(GapKind::Unreached),
                (Some(unready), _) => {
                    Some(GapKind::Unready { form: self.form_at(&key, unready.at()), unready })
                }
                (None, _) => None,
            };
            match kind {
                Some(kind) => coverage.gaps.push(Gap { function: origin, kind }),
                None => coverage.built += 1,
            }
        }
        for (i, lowered) in self.db.get::<EachLowered>(&key).iter() {
            coverage.reached += 1;
            match lowered {
                Ok(()) => coverage.lowered += 1,
                // A literal of this file is counted where MIR stops, above; one of another file
                // is counted here.
                Err(Unlowered { why: Why::Unready(unready), .. }) if i.f.literal.file != key => {
                    let (function, form) =
                        (i.f.literal.expr.origin, self.form_at(&i.f.literal.file, unready.at()));
                    coverage
                        .gaps
                        .push(Gap { function, kind: GapKind::Unready { unready: unready.clone(), form } });
                }
                Err(Unlowered { why: Why::Unready(_), .. }) => {}
                Err(unlowered) => coverage.gaps.push(Gap {
                    function: i.f.literal.expr.origin,
                    kind: GapKind::Unlowered(unlowered.clone()),
                }),
            }
        }
        coverage
    }

    /// Where typing gave up in each body of the program of `file`, reported at the first form of
    /// each: typing gives every form of a body a type, or reports why not, or leaves it to the
    /// instantiations of a generic body.
    pub fn gave_up(&mut self, file: FileId) -> Vec<Diagnostic> {
        let program = self.program(file);
        program
            .bodies()
            .iter()
            .filter_map(|(i, leaves)| {
                let typed = self.db.get::<Typed>(i)?;
                let parsed = self.db.get::<Parsed>(&i.f.literal.file)?;
                typed.gave_up_under(&parsed.hir, i.f.literal.expr.origin, *leaves)
            })
            .collect()
    }

    /// The kind of the form at `at`: an operator by its text, another expression by its kind,
    /// or what else it is.
    fn form_at(&self, file: &FileRef, at: OriginId) -> &'static str {
        let Some(parsed) = self.db.get::<Parsed>(file) else { return "unread" };
        let hir = &parsed.hir;
        if let Some((_, e)) = hir.exprs.iter().find(|(_, e)| e.origin == at) {
            return match &e.kind {
                ExprKind::Prefix { op, .. } | ExprKind::Postfix { op, .. } | ExprKind::Infix { op, .. } => {
                    op.text().unwrap_or(e.kind.name())
                }
                kind => kind.name(),
            };
        }
        if hir.decls.iter().any(|(_, d)| d.origin == at) {
            return "declaration";
        }
        "statement"
    }
}

impl GapKind {
    /// The stage that stops, and the kind of what stops it.
    #[must_use]
    pub fn stage(&self) -> (&'static str, &'static str) {
        match self {
            Self::Unreached => ("reach", "unreached"),
            Self::Unready { form, .. } => ("mir", form),
            Self::Unlowered(unlowered) => ("lir", unlowered.why.name()),
        }
    }
}

impl AddAssign for Coverage {
    fn add_assign(&mut self, other: Self) {
        self.exprs += other.exprs;
        self.typed += other.typed;
        self.bodies += other.bodies;
        self.compile_time += other.compile_time;
        self.built += other.built;
        self.reached += other.reached;
        self.lowered += other.lowered;
        self.gaps.extend(other.gaps);
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percent = |part: usize, whole: usize| (part * 100).checked_div(whole).unwrap_or(100);
        let (typed, exprs, built, bodies) = (self.typed, self.exprs, self.built, self.bodies);
        let (lowered, reached) = (self.lowered, self.reached);
        write!(
            f,
            "sema types {typed} of {exprs} function-body expressions ({}%); MIR builds {built} of {bodies} \
             run-time function literals ({}%, {} more run at compile time); LIR lowers {lowered} of \
             {reached} functions reached ({}%)",
            percent(typed, exprs),
            percent(built, bodies),
            self.compile_time,
            percent(lowered, reached),
        )?;
        let mut kinds: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for gap in &self.gaps {
            *kinds.entry(gap.kind.stage()).or_default() += 1;
        }
        let mut kinds: Vec<_> = kinds.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (i, ((stage, kind), n)) in kinds.into_iter().take(GAPS_SHOWN).enumerate() {
            write!(f, "{}{stage} {kind} {n}", if i == 0 { "; stopped at " } else { ", " })?;
        }
        Ok(())
    }
}
