//! Lints (spec 10, section 4): a file is checked with the lints its package and the packages it
//! depends on, `std` among them by default, define (`#[lint.define]`). Each runs the check it states for a kind of
//! node on each node of that kind in the file, at compile time, through `lang.lint.reports`, a pure
//! call the query engine caches; what it reports is shown at its level, unless a `#[lint.allow]`
//! around the node silences it.

mod defined;
mod levels;
mod literal;
mod locals;
mod nodes;
mod tables;

use std::borrow::Cow;
use std::sync::Arc;

use vun_diag::{DiagCtxt, DiagId, DiagSource, Diagnostic, Fix, Msg, Safety, Severity};
use vun_grammar::PATH_SEPARATOR;
use vun_hir::db::{FileRef, Namespace, Package, PackageId};
use vun_hir::tree::PatKind;
use vun_intern::Symbol;
use vun_items::{Item, lang, std as std_items};
use vun_origin::{OriginId, OriginTable};
use vun_query::{Cx, Query};
use vun_sema_ty::Value;
use vun_sema_ty::call::Instance;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::trap::Unevaluated;
use vun_syntax::make::{Attributes, Expr};

pub use defined::{CheckedLints, DefinedLints};
use levels::Configured;
use nodes::{Kind, Node};
use tables::Level;

use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::Call;
use crate::reflect::Fact;
use crate::visible::Home;

/// The reason an attribute allowing a lint is made with, for its author to write.
const REASON: &str = "why it is allowed";

/// What the lints a file is checked with report of it; `None` for a file the driver did not read.
pub struct Linted;

impl Query for Linted {
    const NAME: &'static str = "linted";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let env = Env::of(cx, file)?;
        let nodes = Node::of(&env);
        let mut diag = DiagCtxt::new();
        let lints = Lint::visible(&env);
        vun_trace::tracing::debug!(file = %file.path, lints = lints.len(), nodes = nodes.len(), "linting");
        for lint in lints {
            lint.run(&env, &nodes, &mut diag);
        }
        Some(Arc::new(diag))
    }
}

/// The member each group of `std.lints` declares its level in.
const GROUP_LEVEL: Item = std_items::lints_naming::level;

/// What a node's kind is shown to checks as: its `lang.lint` struct, and the fact of each field.
const DECL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Decl::name, |_, n| match &n.kind {
        Kind::Decl { name, .. } => Ok(Value::string(name.as_str())),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::binds, |env, n| match &n.kind {
        Kind::Decl { binds, .. } => env.item(binds.item()),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::origin, |_, n| Ok(Value::Origin(n.origin))),
    (lang::lint_Decl::public, |env, n| match &n.kind {
        Kind::Decl { public, .. } => env.decide(*public),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::documented, |env, n| match &n.kind {
        Kind::Decl { documented, .. } => env.decide(*documented),
        _ => Err(Unevaluated::Unsupported),
    }),
];
const LOCAL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Local::name, |_, n| match &n.kind {
        Kind::Local { name, .. } => Ok(Value::string(name.as_str())),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::origin, |_, n| Ok(Value::Origin(n.origin))),
    (lang::lint_Local::used, |env, n| match &n.kind {
        Kind::Local { used, .. } => env.decide(*used),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::param, |env, n| match &n.kind {
        Kind::Local { param, .. } => env.decide(*param),
        _ => Err(Unevaluated::Unsupported),
    }),
];
const CALL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Call::callee, |_, n| match &n.kind {
        Kind::Call { callee } => Ok(callee.clone()),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Call::origin, |_, n| Ok(Value::Origin(n.origin))),
];

/// A lint a file is checked with.
struct Lint {
    /// Its path, its package's name first: `std.lints.naming.case`.
    path: Vec<Symbol>,
    /// Its reports' id: its path.
    id: DiagId,
    /// The group configuring it.
    group: Symbol,
    /// Its value, which `#[lint.allow(lint, ..)]` names.
    value: Value,
    level: Level,
    /// The checks it states, by kind of node; `None` for one left unchecked.
    decl: Option<Value>,
    local: Option<Value>,
    call: Option<Value>,
}

impl Lint {
    /// The lints the file `env` reads is checked with: those its package and the packages it
    /// depends on define, `std` among them by default.
    fn visible(env: &Env<'_, '_>) -> Vec<Self> {
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
        let level = configured.level(&value, members.as_ref(), default);
        let check = |at: Item, node: Item| {
            let check = value.field(at)?.clone();
            let unchecked = call(env, lang::lint::unchecked, vec![env.item(node).ok()?]).ok();
            (Some(&check) != unchecked.as_ref()).then_some(check)
        };
        let path = Self::path(env.cx, decl)?;
        Some(Self {
            id: Self::id_of(&path),
            path,
            group,
            decl: check(lang::lint_Lint::decl, lang::lint::Decl),
            local: check(lang::lint_Lint::local, lang::lint::Local),
            call: check(lang::lint_Lint::call, lang::lint::Call),
            value,
            level,
        })
    }

    /// The id of the reports of the lint at `path`.
    fn id_of(path: &[Symbol]) -> DiagId {
        DiagId(Cow::Owned(path.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(PATH_SEPARATOR)))
    }

    /// The path of the lint `decl` declares.
    fn path(cx: &Cx<'_>, decl: &DeclRef) -> Option<Vec<Symbol>> {
        let env = Env::of(cx, &decl.file)?;
        let Home::Namespace(Namespace { path, .. }) = env.home() else { return None };
        let hir = &env.parsed.hir;
        let PatKind::Bind(binder) = hir.pats[hir.decls[decl.decl.id].pat].kind else { return None };
        let package = cx.get::<Package>(&decl.file.package).name;
        Some(package.iter().chain(&path).chain([&hir.binders[binder].name]).copied().collect())
    }

    /// Runs the lint on `nodes`, the nodes of the file `env` reads, reporting in `diag`.
    fn run(&self, env: &Env<'_, '_>, nodes: &[Node], diag: &mut DiagCtxt) {
        vun_trace::tracing::debug!(lint = %self.id, level = ?self.level, decl = self.decl.is_some(), call = self.call.is_some(), "running");
        let Some(severity) = self.level.severity() else { return };
        let namespace = env.namespace();
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
        let mut d = Diagnostic::new(self.id.clone(), severity, Msg::new().text(msg)).at(at, None);
        let span = OriginTable::global().source_span(at);
        if let Some(Value::Array(fixes)) = report.field(lang::lint_Report::fixes) {
            for fix in fixes {
                let safety = fix.field(lang::lint_Fix::safety).and_then(tables::safety);
                if let (Some(msg), Some(safety), Some(replacing)) =
                    (text(fix, lang::lint_Fix::msg), safety, text(fix, lang::lint_Fix::text))
                {
                    d = d.fix(Fix::replace(Msg::new().text(msg), safety, span, replacing));
                }
            }
        }
        d.source = DiagSource::Lint { group: Cow::Owned(self.group.as_str().to_owned()) };
        let lint = Msg::new().path(self.path.iter().map(|s| s.as_str()));
        let level = lint.clone().text(" reports at ").code(self.level.item().name());
        d.note(level).fix(self.allowing(node.declared))
    }

    /// The fix allowing this lint on the declaration written at `declared`, with a reason to
    /// give: `#[lang.lint.allow(std.lints.naming.case, "...")]`.
    fn allowing(&self, declared: OriginId) -> Fix {
        let path = Expr::Path(self.path.iter().map(|s| s.as_str().to_owned()).collect());
        let allow = Expr::call(Expr::decl(lang::lint::allow), vec![path, Expr::String(REASON.to_owned())]);
        let text = [Attributes(vec![allow]).source(), " ".to_owned()].concat();
        let msg = Msg::new().text("allow it here, saying why");
        Fix::insert(msg, Safety::Placeholder, OriginTable::global().source_span(declared), text)
    }
}

/// The value of the `lang` function `f` called with `args`, all known at compile time.
fn call(env: &Env<'_, '_>, f: Item, args: Vec<Value>) -> Result<Value, Unevaluated> {
    let Value::Fn(f) = env.item(f)? else { return Err(Unevaluated::Unsupported) };
    env.cx.get::<Call>(&Instance { f, args })
}
