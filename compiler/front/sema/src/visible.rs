//! Where names are visible (spec 01, section 7): a name starting with `_` is restricted to its
//! scope, the namespace declaring it unless `#[visible(...)]` widens it. The attribute is the
//! primitive `lang.visible`, and its argument a value of `lang.scope`, so both are read as data.

use vun_codec::{Decode, Encode};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::PATH_SEPARATOR;
use vun_hir::db::{FileRef, Namespace, Package, PackageId};
use vun_hir::tree::{Arg, DeclId, Entry, ExprId, ExprKind, Name};
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};
use vun_syntax::visibility::Visibility;

use crate::Primitive;
use crate::env::Env;
use crate::item::{Declared, ExprRef};
use crate::primitives::lang::Lang;
use crate::primitives::scope::Scope;
use crate::ty::Value;

/// The namespace declaring a member: a file or directory, or the body of a `struct` literal.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Home {
    Namespace(Namespace),
    /// A file outside its package's namespace tree, such as a test program: its own scope.
    File(FileRef),
    Struct(ExprRef),
}

/// Where a name is visible, besides the namespaces nested in it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Reach {
    Public,
    /// Its home.
    Home,
    Within(Namespace),
    Package(PackageId),
    Workspace,
}

/// A member's visibility: how far it reaches from where it is declared.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Visible {
    pub reach: Reach,
    pub home: Home,
}

/// A `#[visible(...)]` that cannot apply: on a public name, or naming a namespace that does not
/// enclose the declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Misplaced {
    Public,
    NotEnclosing,
}

impl Env<'_, '_> {
    /// The home of this file's top-level declarations: the file's namespace, or the file.
    #[must_use]
    pub fn home(&self) -> Home {
        match self.cx.get::<Package>(&self.file.package).namespace_of(self.file.path.as_str()) {
            Some(path) => Home::Namespace(Namespace { package: self.file.package, path }),
            None => Home::File(self.file.clone()),
        }
    }

    /// The visibility of the member `name` with attributes `attrs`, declared at `home` in this
    /// file, and the `#[visible(...)]` among `attrs` that cannot apply, if any.
    #[must_use]
    pub fn visible(
        &self,
        name: Symbol,
        attrs: &[ExprId],
        home: Home,
    ) -> (Visible, Option<(ExprId, Misplaced)>) {
        let widening = attrs.iter().find_map(|&attr| Some((attr, self.widening(attr)?)));
        let restricted = Visibility::of(name.as_str()) == Visibility::Restricted;
        let (reach, misplaced) = match widening {
            None if restricted => (Reach::Home, None),
            None => (Reach::Public, None),
            Some((attr, _)) if !restricted => (Reach::Public, Some((attr, Misplaced::Public))),
            Some((attr, Reach::Within(ns))) if !self.encloses(&ns, &home) => {
                (Reach::Home, Some((attr, Misplaced::NotEnclosing)))
            }
            Some((_, reach)) => (reach, None),
        };
        (Visible { reach, home }, misplaced)
    }

    /// The scope a `#[visible(scope)]` attribute at `attr` names; `None` for any other attribute.
    fn widening(&self, attr: ExprId) -> Option<Reach> {
        let ExprKind::Call { callee, args } = &self.parsed.hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::visible))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        self.scope(*value)
    }

    /// A `lang.scope.Scope` written `.package`, `.workspace` or `.within(ns)`: `.name` is looked
    /// up in `lang.scope`, the namespace declaring the expected type (spec 01, section 2).
    fn scope(&self, expr: ExprId) -> Option<Reach> {
        let hir = &self.parsed.hir;
        let (name, arg) = match &hir.exprs[expr].kind {
            ExprKind::DotName(Name::Plain(name)) => (*name, None),
            ExprKind::Call { callee, args } => match (&hir.exprs[*callee].kind, args.as_slice()) {
                (ExprKind::DotName(Name::Plain(name)), [Entry::One(Arg { name: None, value })]) => {
                    (*name, Some(*value))
                }
                _ => return None,
            },
            _ => return None,
        };
        let Ok(Value::Namespace(scopes)) = self.item(Scope::NAMESPACE) else { return None };
        let Ok(Value::Primitive(Primitive::Scope(scope))) =
            self.value_of(self.cx.get::<Declared>(&(scopes, name))?)
        else {
            return None;
        };
        match (scope, arg.map(|a| self.eval(a))) {
            (Scope::package, None) => Some(Reach::Package(self.file.package)),
            (Scope::workspace, None) => Some(Reach::Workspace),
            (Scope::within, Some(Ok(Value::Namespace(ns)))) => Some(Reach::Within(ns)),
            _ => None,
        }
    }

    /// Whether the namespace `ns` encloses `home`, or is it.
    fn encloses(&self, ns: &Namespace, home: &Home) -> bool {
        match home {
            Home::Namespace(home) => home.package == ns.package && home.path.starts_with(&ns.path),
            Home::File(file) | Home::Struct(ExprRef { file, .. }) => {
                let package = self.cx.get::<Package>(&file.package);
                file.package == ns.package
                    && package.namespace_of(file.path.as_str()).is_some_and(|path| path.starts_with(&ns.path))
            }
        }
    }

    /// The visibility of `name`, bound by the top-level declaration `decl` of this file.
    #[must_use]
    pub fn decl_visible(&self, decl: DeclId, name: Symbol) -> Visible {
        self.visible(name, &self.parsed.hir.decls[decl].attrs, self.home()).0
    }

    /// The report of `name`, used at `at` outside where `visible` reaches.
    #[must_use]
    pub fn not_visible(&self, at: OriginId, name: Symbol, visible: &Visible) -> Diagnostic {
        let msg = Msg::new().code(name.as_str()).text(" is not visible here");
        let note = Msg::new().text("it is visible only inside ").append(self.scope_msg(visible));
        Diagnostic::error(errors::NAMES_PRIVATE, msg).at(at, None).child(ChildKind::Note, note)
    }

    /// Where `visible` reaches, for diagnostics: a namespace, a type, the package or the
    /// workspace.
    #[must_use]
    pub fn scope_msg(&self, visible: &Visible) -> Msg {
        let path = |ns: &Namespace| {
            let names: Vec<_> = ns.path.iter().map(|n| n.as_str()).collect();
            Msg::new().code(names.join(PATH_SEPARATOR))
        };
        match (&visible.reach, &visible.home) {
            (Reach::Home, Home::Namespace(ns)) | (Reach::Within(ns), _) => path(ns),
            (Reach::Home, Home::File(file)) => Msg::new().text("the file ").code(file.path.as_str()),
            (Reach::Home, Home::Struct(literal)) => {
                let origins = OriginTable::global();
                let name = Env::of(self.cx, &literal.file)
                    .and_then(|env| origins.declaration_name(env.parsed.hir.exprs[literal.expr.id].origin));
                name.map_or_else(|| Msg::new().text("its struct"), |n| Msg::new().code(n.as_str()))
            }
            (Reach::Package(_), _) => Msg::new().text("its package"),
            (Reach::Workspace, _) => Msg::new().text("its workspace"),
            (Reach::Public, _) => Msg::new().text("every namespace"),
        }
    }

    /// Whether a member with visibility `visible` can be used at `at`, an expression of this
    /// file; `None` when that is not known, as for the workspace, which packages do not record
    /// yet.
    #[must_use]
    pub fn sees(&self, visible: &Visible, at: ExprId) -> Option<bool> {
        let here = self.home();
        Some(match &visible.reach {
            Reach::Public => true,
            Reach::Home => match &visible.home {
                Home::Namespace(ns) => self.encloses(ns, &here),
                Home::File(file) => *file == self.file,
                Home::Struct(literal) => {
                    let origins = OriginTable::global();
                    let span = |file: &ExprRef| {
                        let env = Env::of(self.cx, &file.file)?;
                        Some(origins.source_span(env.parsed.hir.exprs[file.expr.id].origin))
                    };
                    let (outer, inner) =
                        (span(literal)?, span(&ExprRef::new(self.file.clone(), &self.parsed.hir, at))?);
                    outer.file == inner.file && outer.range.contains(inner.range)
                }
            },
            Reach::Within(ns) => self.encloses(ns, &here),
            Reach::Package(package) => *package == self.file.package,
            Reach::Workspace => return None,
        })
    }
}
