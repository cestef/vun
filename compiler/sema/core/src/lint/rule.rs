//! A lint as a file is checked with it (spec 10, section 4): what it is, its level, and its
//! checks, read from the `lang.lint.Lint` value its declaration holds and run on a file's nodes.

use vun_diag::{DiagCtxt, DiagId, DiagSource, Diagnostic, Fix, Msg, Safety, Severity, errors};
use vun_hir::db::{Namespace, Package, PackageId};
use vun_hir::tree::PatKind;
use vun_intern::Symbol;
use vun_items::{Item, lang, std as std_items};
use vun_origin::{OriginId, OriginTable};
use vun_query::Cx;
use vun_sema_ty::Value;
use vun_sema_ty::call::Instance;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::trap::{Edit, Unevaluated};
use vun_span::{SourceFiles, SourceMap as _, Span};
use vun_syntax::make::{Attributes, Expr};

use super::DefinedLints;
use super::levels::Configured;
use super::nodes::{CALL, DECL, Kind, LOCAL, Node};
use super::tables::Level;
use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::Call;
use crate::visible::Home;

/// The reason an attribute allowing a lint is made with, for its author to write.
const REASON: &str = "why it is allowed";

/// The member each group of `std.lints` declares its level in.
const GROUP_LEVEL: Item = std_items::lints_naming::level;

/// A lint a file is checked with.
pub(super) struct Lint {
    /// Its path, its package's name first: `std.lints.naming.case`.
    path: Vec<Symbol>,
    /// Its reports' id: its path.
    id: DiagId,
    /// The group configuring it.
    group: Symbol,
    /// Its value, which `#[lint.allow(lint, ..)]` names.
    value: Value,
    pub(super) level: Level,
    /// The checks it states, by kind of node; `None` for one left unchecked.
    decl: Option<Value>,
    local: Option<Value>,
    call: Option<Value>,
}

impl Lint {
    /// The lints the file `env` reads is checked with: those its package and the packages it
    /// depends on define, `std` among them by default.
    pub(super) fn visible(env: &Env<'_, '_>) -> Vec<Self> {
        let cx = env.cx;
        let data = cx.get::<Package>(&env.file.package);
        let deps = data.deps.iter().filter_map(|d| d.package.as_ref().ok().copied());
        let mut packages: Vec<PackageId> = std::iter::once(env.file.package).chain(deps).collect();
        packages.sort();
        packages.dedup();
        let defined = packages.iter().flat_map(|p| cx.get::<DefinedLints>(p).as_ref().clone());
        let configured = Configured::of(env);
        defined
            .filter_map(|decl| {
                let lint = Self::read(env, &decl, &configured);
                if lint.is_none() {
                    vun_trace::tracing::debug!(at = %decl.decl.origin.located(), "a lint that does not read");
                }
                lint
            })
            .collect()
    }

    /// The lint the declaration `decl` defines, as it is seen from the file `env` reads, at the
    /// level its package's build configures.
    fn read(env: &Env<'_, '_>, decl: &DeclRef, configured: &Configured) -> Option<Self> {
        let value = env.cx.get::<DeclValue>(decl).ok()?;
        let Value::Name(group) = *value.field(lang::lint_Lint::group)? else { return None };
        let members = env.item(std_items::lints).and_then(|lints| env.member(lints, group)).ok();
        let default = match value.field(lang::lint_Lint::default)? {
            Value::Variant { payload, .. } if !payload.is_empty() => Level::of(&payload[0])?,
            _ => Level::of(&env.member(members.clone()?, GROUP_LEVEL.symbol()).ok()?)?,
        };
        let default = if Self::foreign(env, decl) { default.capped() } else { default };
        let path = Self::path(env.cx, decl)?;
        let id = Self::id_of(&path);
        let group_id = Self::group_id(group);
        let level = configured.level(&value, &id, members.as_ref(), &group_id, default);
        let check = |at: Item, node: Item| {
            let check = value.field(at)?.clone();
            let unchecked = call(env, lang::lint::unchecked, vec![env.item(node).ok()?]).ok();
            (Some(&check) != unchecked.as_ref()).then_some(check)
        };
        Some(Self {
            id,
            path,
            group,
            decl: check(lang::lint_Lint::decl, lang::lint::Decl),
            local: check(lang::lint_Lint::local, lang::lint::Local),
            call: check(lang::lint_Lint::call, lang::lint::Call),
            value,
            level,
        })
    }

    /// Whether the lint `decl` defines belongs to a package other than the one of the file it is
    /// read for and the standard one (spec 10, section 4): a dependency's lint reports at most
    /// `warn` while a dependent is built.
    fn foreign(env: &Env<'_, '_>, decl: &DeclRef) -> bool {
        if decl.file.package == env.file.package {
            return false;
        }
        let defining = env.cx.get::<Package>(&decl.file.package);
        defining.name.map(Symbol::as_str) != Some(vun_items::std.name())
    }

    /// The id of the reports of the lint at `path`.
    pub(super) fn id_of(path: &[Symbol]) -> DiagId {
        Self::id_of_strs(path.iter().map(|s| s.as_str()))
    }

    /// The id of the group of lints `group` names, a member of `std.lints`.
    pub(super) fn group_id(group: Symbol) -> DiagId {
        Self::id_of_strs(std_items::lints.path().iter().copied().chain([group.as_str()]))
    }

    /// The id of a binding path of `segments`.
    fn id_of_strs<'a>(segments: impl IntoIterator<Item = &'a str>) -> DiagId {
        DiagId::package(segments.into_iter().map(Symbol::intern))
    }

    /// The path of the lint `decl` declares.
    pub(super) fn path(cx: &Cx<'_>, decl: &DeclRef) -> Option<Vec<Symbol>> {
        let env = Env::of(cx, &decl.file)?;
        let Home::Namespace(Namespace { path, .. }) = env.home() else { return None };
        let hir = &env.parsed.hir;
        let PatKind::Bind(binder) = hir.pats[hir.decls[decl.decl.id].pat].kind else { return None };
        let package = cx.get::<Package>(&decl.file.package).name;
        Some(package.iter().chain(&path).chain([&hir.binders[binder].name]).copied().collect())
    }

    /// Runs the lint on `nodes`, the nodes of the file `env` reads, reporting in `diag`; whether
    /// it reported anything.
    pub(super) fn run(&self, env: &Env<'_, '_>, nodes: &[Node], diag: &mut DiagCtxt) -> bool {
        vun_trace::tracing::debug!(lint = %self.id, level = ?self.level, decl = self.decl.is_some(), call = self.call.is_some(), "running");
        let Some(severity) = self.level.severity() else { return false };
        let namespace = env.namespace();
        let mut fired = false;
        // `forbid` is final: no attribute allows the lint.
        let allowed = |n: &&Node| self.level != Level::Forbid && n.allowed.contains(&self.value);
        for node in nodes.iter().filter(|n| !allowed(n)) {
            let (check, shape, facts) = match node.kind {
                Kind::Decl { .. } => (&self.decl, lang::lint::Decl, DECL),
                Kind::Local { .. } => (&self.local, lang::lint::Local, LOCAL),
                Kind::Call { .. } => (&self.call, lang::lint::Call, CALL),
            };
            let Some(check) = check else { continue };
            let Ok(Value::Array(mut shown)) = env.describe(shape, std::slice::from_ref(node), facts) else {
                continue;
            };
            let (Some(shown), Ok(ty)) = (shown.pop(), env.item(shape)) else { continue };
            let args = vec![ty, check.clone(), namespace.clone(), shown];
            match call(env, lang::lint::reports, args) {
                Ok(Value::Array(reports)) => {
                    for report in &reports {
                        diag.report(self.diagnostic(report, node, severity));
                        fired = true;
                    }
                }
                Err(Unevaluated::Trapped(trap)) => {
                    diag.error(trap.report(node.origin));
                }
                Ok(_) => {}
                Err(why) => {
                    let stopped = match why {
                        Unevaluated::Unbuilt(at) => Some(OriginTable::global().source_span(at)),
                        _ => None,
                    };
                    vun_trace::tracing::debug!(lint = %self.id, ?why, ?stopped, "no reports");
                }
            }
        }
        fired
    }

    /// The report that the lint was expected and did not fire: what a lint at `expect` says when
    /// it reports nothing (spec 10, section 4).
    pub(super) fn unfulfilled(&self, env: &Env<'_, '_>) -> Diagnostic {
        let msg = Msg::new()
            .text("expected ")
            .path(self.path.iter().map(|s| s.as_str()))
            .text(" to report, and it did not");
        let help = Msg::new().text("remove the expectation, or fix what silences the lint");
        let d = Diagnostic::new(errors::LINT_UNFULFILLED, Severity::Warning, msg).help(help);
        match env.parsed.hir.root {
            Some(root) => d.at(env.parsed.hir.namespaces[root].origin, None),
            None => d,
        }
    }

    /// The diagnostic `report`, a `lang.lint.Report`, of `node`: what the lint says, with its
    /// fixes, the level it reports at, and the fix allowing it on the declaration around.
    fn diagnostic(&self, report: &Value, node: &Node, severity: Severity) -> Diagnostic {
        let at = match report.field(lang::lint_Report::at) {
            Some(Value::Origin(at)) => *at,
            _ => node.origin,
        };
        let text = |f: &Value, at: Item| f.field(at).and_then(Value::text);
        let msg = text(report, lang::lint_Report::msg).unwrap_or_default();
        let mut d = Diagnostic::new(self.id, severity, Msg::new().text(msg)).at(at, None);
        let span = OriginTable::global().source_span(at);
        if let Some(Value::Array(fixes)) = report.field(lang::lint_Report::fixes) {
            for edit in fixes.iter().filter_map(Edit::of) {
                d = d.fix(edit.fix(span));
            }
        }
        d.source = DiagSource::Lint { group: self.group };
        let lint = Msg::new().path(self.path.iter().map(|s| s.as_str()));
        let level = lint.clone().text(" reports at ").code(self.level.item().name());
        d.note(level).fix(self.allowing(node.declared))
    }

    /// The fix allowing this lint on the declaration written at `declared`, with a reason to
    /// give: `#[lang.lint.allow(std.lints.naming.case, "...")]`.
    fn allowing(&self, declared: OriginId) -> Fix {
        let path = Expr::Path(self.path.iter().map(|s| s.as_str().to_owned()).collect());
        let allow = Expr::call(Expr::decl(lang::lint::allow), vec![path, Expr::String(REASON.to_owned())]);
        // An attribute is wide: it goes on a line of its own above the declaration, at its indent.
        let span = OriginTable::global().source_span(declared);
        let indent = Self::indent_before(span).unwrap_or_default();
        let separator =
            if Self::indent_before(span).is_some() { format!("\n{indent}") } else { " ".to_owned() };
        let text = [Attributes(vec![allow]).source(), separator].concat();
        let msg = Msg::new().text("allow it here, saying why");
        Fix::insert(msg, Safety::Placeholder, span, text)
    }

    /// The indentation of the line `span` starts, when nothing but it precedes `span` there.
    fn indent_before(span: Span) -> Option<String> {
        let text = &SourceFiles::global().file(span.file).text;
        let before = text.get(..usize::try_from(span.range.lo).ok()?)?;
        let line = before.rsplit('\n').next()?;
        line.chars().all(|c| c == ' ' || c == '\t').then(|| line.to_owned())
    }
}

/// The value of the `lang` function `f` called with `args`, all known at compile time.
fn call(env: &Env<'_, '_>, f: Item, args: Vec<Value>) -> Result<Value, Unevaluated> {
    let Value::Fn(f) = env.item(f)? else { return Err(Unevaluated::Unsupported) };
    env.cx.get::<Call>(&Instance { f, args })
}
