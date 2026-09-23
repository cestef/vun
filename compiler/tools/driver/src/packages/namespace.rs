//! Namespaces and what they export, read from disk; a file's declarations are read once per
//! content.

use std::path::Path;

use overt_build::layout::Location;
use overt_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use overt_hir::tree::{Hir, Member, each_entry};
use overt_intern::Symbol;
use overt_span::{FileId, SourceMap};
use overt_syntax::visibility::Visibility;

use super::PackageKey;
use crate::Session;

/// A package's root namespace, or one below it by member names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Namespace {
    pub package: PackageKey,
    pub path: Vec<Symbol>,
}

/// Why a path names no namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfound {
    /// Its first name is not the package or a dependency.
    Root,
    /// The dependency it starts at cannot be loaded, which is reported.
    Dep(ErrorGuaranteed),
    /// Nothing on disk at the path.
    Absent,
}

impl std::fmt::Display for Unfound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Root => "the path does not start at the package or a dependency",
            Self::Dep(_) => "the dependency it starts at cannot be loaded",
            Self::Absent => "nothing is on disk at the path",
        })
    }
}

impl std::error::Error for Unfound {}

/// What a file contributes to its namespace: its top-level names, and the paths it spreads whole.
#[derive(Clone, Default)]
pub(super) struct Declared {
    names: Vec<Symbol>,
    spreads: Vec<Vec<Symbol>>,
}

impl Session {
    /// The namespace at `path` as seen from `package`: its first name is the package itself or a
    /// dependency (spec 01, section 8).
    ///
    /// # Errors
    /// Returns why no namespace is there.
    pub fn namespace_at(&mut self, package: PackageKey, path: &[Symbol]) -> Result<Namespace, Unfound> {
        let (&first, rest) = path.split_first().ok_or(Unfound::Root)?;
        let p = &self.packages.list[&package];
        let root = if p.name == Some(first) {
            package
        } else {
            let index = p.deps.iter().position(|d| d.name == first).ok_or(Unfound::Root)?;
            self.dep(package, index).map_err(Unfound::Dep)?
        };
        let ns = Namespace { package: root, path: rest.to_vec() };
        self.location(&ns).filter(Location::exists).map(|_| ns).ok_or(Unfound::Absent)
    }

    /// The public names of `ns`, which other namespaces see.
    pub fn exports(&mut self, ns: &Namespace) -> Vec<Symbol> {
        let mut names = self.members(ns);
        names.retain(|&n| Visibility::of(n.as_str()) == Visibility::Public);
        names
    }

    /// The names of `ns`: declared in its file, exported by what it spreads whole, and held in its
    /// directory. A namespace reached again while its own spreads are read contributes nothing
    /// more.
    pub fn members(&mut self, ns: &Namespace) -> Vec<Symbol> {
        let Some(location) = self.location(ns) else { return Vec::new() };
        if !self.packages.expanding.insert(location.clone()) {
            return Vec::new();
        }
        let declared = location.file().and_then(|path| self.declared(&path)).unwrap_or_default();
        let mut names = declared.names;
        for path in &declared.spreads {
            if let Ok(target) = self.namespace_at(ns.package, path) {
                names.extend(self.exports(&target));
            }
        }
        names.extend(location.children().iter().map(|c| Symbol::intern(c)));
        self.packages.expanding.remove(&location);
        names
    }

    /// Reports the top-level names of `file` that its directory also holds: both are members of
    /// one namespace.
    pub(crate) fn check_children(&mut self, file: FileId, hir: &Hir) {
        let Some(root) = hir.root else { return };
        let location = Location::of(Path::new(&self.sources.file(file).path));
        let children = location.children();
        for b in hir.members(root) {
            let name = hir.binders[b].name.as_str();
            if children.iter().any(|c| c == name) {
                let dir = location.dir().display().to_string();
                let msg = Msg::new().code(name).text(" is declared here and also held in ").code(dir);
                let d = Diagnostic::error(errors::NAMES_DUPLICATE, msg);
                self.diag.error(d.at(hir.binders[b].origin, None));
            }
        }
    }

    fn location(&self, ns: &Namespace) -> Option<Location> {
        let root = self.packages.list[&ns.package].root()?;
        Some(ns.path.iter().fold(root, |l, &s| l.member(s.as_str())))
    }

    /// What the file at `path` declares at its top level, read once per content.
    fn declared(&mut self, path: &Path) -> Option<Declared> {
        self.by_content(
            path,
            |p| &mut p.declared,
            |s, file| {
                let tokens = s.lex(file);
                let cst = s.parse(&tokens);
                let hir = s.lower(&tokens, &cst);
                let mut declared = Declared::default();
                if let Some(root) = hir.root {
                    declared.names = hir.members(root).into_iter().map(|b| hir.binders[b].name).collect();
                    each_entry(&hir.namespaces[root].members, &mut |m| {
                        if let Member::Spread(s) = m
                            && s.select.is_none()
                            && let Some((_, path)) = hir.path(s.path)
                        {
                            declared.spreads.push(path);
                        }
                    });
                }
                declared
            },
        )
    }
}
