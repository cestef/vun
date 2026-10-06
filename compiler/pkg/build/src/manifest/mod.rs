//! The `package` literal of a `build.vun` (spec 12, section 2), read without evaluating anything.
//!
//! Every field is checked: missing required fields, unknown fields (with the closest known name)
//! and values of the wrong shape are diagnostics at the offending source.

mod deps;
mod identity;
pub(crate) mod record;
#[cfg(test)]
mod tests;
mod version;
mod write;

use vun_diag::{ChildKind, DiagCx, Diagnostic, Msg, errors};
use vun_span::{FileId, Span};
use vun_syntax::literal::Decl;

pub use identity::Identity;
use record::{Fields, Record};
pub use version::Version;
use vun_hash::Hash;
use vun_target::{Feature, Features};
use vun_uri::Uri;

use crate::layout::MANIFEST;

/// The declaration `std.build.Package` requires to hold the manifest.
pub const PACKAGE: &str = "package";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    /// Chosen once by `vun new`; survives renames.
    pub id: u64,
    pub version: Version,
    pub edition: u16,
    /// The exact toolchain the launcher runs, if pinned.
    pub toolchain: Option<Version>,
    /// The components the pinned toolchain builds the package with, by name (spec 12, section 6):
    /// installed with it when missing.
    pub components: Vec<String>,
    /// The oldest toolchain that reads this manifest and builds the package.
    pub requires: Option<Version>,
    /// The language features the package enables (spec 12, section 2).
    pub features: Features,
    /// The path in the package `targets` the package is written for, as `--target` takes it;
    /// checking or building one of its files chooses it instead of the machine's.
    pub target: Option<String>,
    /// What the content hash covers.
    pub include: Vec<String>,
    pub deps: Vec<Dep>,
    /// Identities whose packages come from elsewhere, anywhere in the graph; read from the root.
    pub replace: Vec<Replacement>,
    /// Indexes that list newer versions, tried in order; read from the root (spec 12, section 3).
    pub index: Vec<Uri>,
    /// Workspace members, by path.
    pub members: Vec<String>,
    /// Namespaces whose public names every file sees unqualified (spec 01, section 8).
    pub prelude: Vec<String>,
    /// The `.prelude` field, or the `package` declaration when the prelude is the default.
    pub prelude_span: Span,
}

/// The edition new packages are written in.
pub const EDITION: u16 = 2026;

/// The prelude of a manifest that does not name one.
pub const DEFAULT_PRELUDE: &str = "std.prelude";

/// Dependencies every package has unless it declares its own under the same name (spec 01,
/// section 8), found in the toolchain's library.
pub const DEFAULT_DEPS: &[&str] = &["lang", "std", "targets"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub source: Source,
    pub kind: DepKind,
    pub span: Span,
}

/// `.replace` entry: the identity whose every version comes from `source` instead (spec 12,
/// section 3), named for people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    pub name: String,
    pub identity: Identity,
    pub source: Source,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A released version, fetched from any mirror and checked against its hash.
    Archive { version: Version, hash: Hash, urls: Vec<Uri> },
    /// A package next to this one, by path.
    Path(String),
    /// A package of the toolchain's library, by its path inside it (`boards/rp2350`): the same
    /// package on every machine, whichever toolchain is installed.
    Library(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepKind {
    /// A Vun package with its own `build.vun`.
    Vun,
    /// A source archive without one, compiled by the dependent's build (spec 12, section 8).
    Foreign,
}

const DEP_KINDS: &[(&str, DepKind)] = &[("vun", DepKind::Vun), ("foreign", DepKind::Foreign)];

mod field {
    pub(super) const NAME: &str = "name";
    pub(super) const ID: &str = "id";
    pub(super) const VERSION: &str = "version";
    pub(super) const EDITION: &str = "edition";
    pub(super) const TOOLCHAIN: &str = "toolchain";
    pub(super) const COMPONENTS: &str = "components";
    pub(super) const REQUIRES: &str = "requires";
    pub(super) const TARGET: &str = "target";
    pub(super) const INCLUDE: &str = "include";
    pub(super) const DEPS: &str = "deps";
    pub(super) const MEMBERS: &str = "members";
    pub(super) const PRELUDE: &str = "prelude";
    pub(super) const FEATURES: &str = "features";
    pub(super) const HASH: &str = "hash";
    pub(super) const URLS: &str = "urls";
    pub(super) const PATH: &str = "path";
    pub(super) const LIBRARY: &str = "library";
    pub(super) const KIND: &str = "kind";
    pub(super) const REPLACE: &str = "replace";
    pub(super) const MAJOR: &str = "major";
    pub(super) const INDEX: &str = "index";

    pub(super) const PACKAGE: &[&str] = &[
        NAME, ID, VERSION, EDITION, TOOLCHAIN, COMPONENTS, REQUIRES, TARGET, INCLUDE, DEPS, REPLACE, INDEX,
        MEMBERS, PRELUDE, FEATURES,
    ];
    pub(super) const DEP: &[&str] = &[VERSION, HASH, URLS, PATH, LIBRARY, KIND];
    pub(super) const REPLACEMENT: &[&str] = &[ID, MAJOR, VERSION, HASH, URLS, PATH, LIBRARY];
}

impl Manifest {
    /// Whether a `build.vun` holds a manifest: it declares `package`. One that does not is an ordinary
    /// source file, such as the namespace `std.build`.
    #[must_use]
    pub fn declared(decls: &[Decl<'_>]) -> bool {
        decls.iter().any(|d| d.name == PACKAGE)
    }

    /// The files a package's hash covers, its manifest among them, so that what the package
    /// declares is verified with its contents.
    fn include(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<String>> {
        let include = decl.paths(cx)?;
        if include.iter().any(|path| path == MANIFEST) {
            return Some(include);
        }
        let help = Msg::new().text("a package's hash covers its manifest");
        decl.invalid_with(Msg::new().text("a list naming ").code(MANIFEST), Some(help), cx)
    }

    /// The toolchain that is reading: this build's version.
    ///
    /// # Panics
    /// If the workspace's version is not `major.minor.patch`.
    #[must_use]
    pub fn running() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).expect("the workspace's version is one")
    }

    /// Reports a manifest that asks for a newer toolchain than the one reading it; its other
    /// fields may be ones this toolchain does not know, so they are not read.
    fn outdated(fields: &Fields<'_, '_>, cx: &mut DiagCx<'_>) -> bool {
        let Some(decl) = fields.get(field::REQUIRES) else { return false };
        let Some(required) = decl.version(cx) else { return false };
        let running = Self::running();
        if required <= running {
            return false;
        }
        let msg = Msg::new()
            .text("this package requires toolchain ")
            .code(required.to_string())
            .text(", this is ")
            .code(running.to_string());
        let help = Msg::new().text("update the toolchain with ").code("vun toolchain update");
        let diag = Diagnostic::error(errors::MANIFEST_TOOLCHAIN_TOO_OLD, msg).child(ChildKind::Help, help);
        cx.error_at(decl.span, diag);
        true
    }

    /// Reads the manifest from the declarations of a `build.vun`; `file` locates a missing `package`.
    pub fn read(decls: &[Decl<'_>], file: FileId, cx: &mut DiagCx<'_>) -> Option<Self> {
        let Some(package) = decls.iter().find(|d| d.name == PACKAGE) else {
            let msg = Msg::new().text("no ").code(PACKAGE).text(" declaration");
            let help = Msg::new().text("declare ").code(format!("{PACKAGE} := .{{ .name = \"...\", ... }};"));
            let d = Diagnostic::error(errors::MANIFEST_MISSING_PACKAGE, msg).child(ChildKind::Help, help);
            cx.error_at(Span::new(file, 0, 0), d);
            return None;
        };
        let mut fields = Fields::new(package, field::PACKAGE, cx)?;
        if Self::outdated(&fields, cx) {
            return None;
        }
        // Every field is read before any result is used, so that one run reports every problem.
        let name = fields.require(field::NAME).and_then(|d| d.string(cx));
        let id = fields.require(field::ID).and_then(|d| d.int(cx));
        let version = fields.require(field::VERSION).and_then(|d| d.version(cx));
        let edition = fields.require(field::EDITION).and_then(|d| d.int(cx));
        let toolchain = fields.get(field::TOOLCHAIN).map_or(Some(None), |d| d.version(cx).map(Some));
        let components = fields.get(field::COMPONENTS).map_or(Some(Vec::new()), |d| d.strings(cx));
        let requires = fields.get(field::REQUIRES).map_or(Some(None), |d| d.version(cx).map(Some));
        let features = fields.get(field::FEATURES).map_or(Some(Features::default()), |d| {
            let table: Vec<_> = Feature::ALL.iter().map(|&f| (f.name(), f)).collect();
            d.choices(&table, cx).map(Features::of)
        });
        let target = fields.get(field::TARGET).map_or(Some(None), |d| d.string(cx).map(Some));
        let include = fields.require(field::INCLUDE).and_then(|d| Self::include(d, cx));
        let deps = fields.get(field::DEPS).map_or(Some(Vec::new()), |d| Dep::list(d, cx));
        let replace = fields.get(field::REPLACE).map_or(Some(Vec::new()), |d| Replacement::list(d, cx));
        let index = fields.get(field::INDEX).map_or(Some(Vec::new()), |d| d.uris(cx));
        let members = fields.get(field::MEMBERS).map_or(Some(Vec::new()), |d| d.strings(cx));
        let prelude_decl = fields.get(field::PRELUDE);
        let prelude = prelude_decl.map_or(Some(vec![DEFAULT_PRELUDE.to_owned()]), |d| d.strings(cx));
        let prelude_span = prelude_decl.unwrap_or(package).span;
        fields.finish(cx);
        Some(Self {
            name: name?,
            id: id?,
            version: version?,
            edition: edition?,
            toolchain: toolchain?,
            components: components?,
            features: features?,
            requires: requires?,
            target: target?,
            include: include?,
            deps: deps?,
            replace: replace?,
            index: index?,
            members: members?,
            prelude: prelude?,
            prelude_span,
        })
    }
}
