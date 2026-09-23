//! The `package` literal of a `build.ov` (spec 12, section 2), read without evaluating anything.
//!
//! Every field is checked: missing required fields, unknown fields (with the closest known name)
//! and values of the wrong shape are diagnostics at the offending source.

mod record;
#[cfg(test)]
mod tests;
mod version;

use overt_diag::{DiagCx, Diagnostic, Msg, errors};
use overt_span::{FileId, Span};
use overt_syntax::literal::{Decl, Literal};

use overt_hash::Hash;
use record::{Fields, choice, hash, int, string, strings};
pub use version::Version;

/// The declaration `std.build.Package` requires to hold the manifest.
pub const PACKAGE: &str = "package";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    /// Chosen once by `overt new`; survives renames.
    pub id: u64,
    pub version: Version,
    pub edition: u16,
    /// The exact toolchain the launcher runs, if pinned.
    pub toolchain: Option<Version>,
    /// What the content hash covers.
    pub include: Vec<String>,
    pub deps: Vec<Dep>,
    /// Workspace members, by path.
    pub members: Vec<String>,
    /// Namespaces whose public names every file sees unqualified (spec 01, section 8).
    pub prelude: Vec<String>,
    /// The `.prelude` field, or the `package` declaration when the prelude is the default.
    pub prelude_span: Span,
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A released version, fetched from any mirror and checked against its hash.
    Archive { version: Version, hash: Hash, urls: Vec<String> },
    /// A package next to this one, by path.
    Path(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepKind {
    /// An Overt package with its own `build.ov`.
    Overt,
    /// A source archive without one, compiled by the dependent's build (spec 12, section 8).
    Foreign,
}

const DEP_KINDS: &[(&str, DepKind)] = &[("overt", DepKind::Overt), ("foreign", DepKind::Foreign)];

mod field {
    pub(super) const NAME: &str = "name";
    pub(super) const ID: &str = "id";
    pub(super) const VERSION: &str = "version";
    pub(super) const EDITION: &str = "edition";
    pub(super) const TOOLCHAIN: &str = "toolchain";
    pub(super) const INCLUDE: &str = "include";
    pub(super) const DEPS: &str = "deps";
    pub(super) const MEMBERS: &str = "members";
    pub(super) const PRELUDE: &str = "prelude";
    pub(super) const HASH: &str = "hash";
    pub(super) const URLS: &str = "urls";
    pub(super) const PATH: &str = "path";
    pub(super) const KIND: &str = "kind";

    pub(super) const PACKAGE: &[&str] =
        &[NAME, ID, VERSION, EDITION, TOOLCHAIN, INCLUDE, DEPS, MEMBERS, PRELUDE];
    pub(super) const DEP: &[&str] = &[VERSION, HASH, URLS, PATH, KIND];
}

/// Whether a `build.ov` holds a manifest: it declares `package`. One that does not is an ordinary
/// source file, such as the namespace `std.build`.
#[must_use]
pub fn declared(decls: &[Decl<'_>]) -> bool {
    decls.iter().any(|d| d.name == PACKAGE)
}

/// Reads the manifest from the declarations of a `build.ov`; `file` locates a missing `package`.
pub fn read(decls: &[Decl<'_>], file: FileId, cx: &mut DiagCx<'_>) -> Option<Manifest> {
    let Some(package) = decls.iter().find(|d| d.name == PACKAGE) else {
        let msg = Msg::new().text("no ").code(PACKAGE).text(" declaration");
        let help = Msg::new().text("declare ").code(format!("{PACKAGE} := .{{ .name = \"...\", ... }};"));
        let d =
            Diagnostic::error(errors::MANIFEST_MISSING_PACKAGE, msg).child(overt_diag::ChildKind::Help, help);
        cx.error_at(Span::new(file, 0, 0), d);
        return None;
    };
    let mut fields = Fields::new(package, field::PACKAGE, cx)?;
    // Every field is read before any result is used, so that one run reports every problem.
    let name = fields.require(field::NAME).and_then(|d| string(d, cx));
    let id = fields.require(field::ID).and_then(|d| int(d, cx));
    let version = fields.require(field::VERSION).and_then(|d| Version::read(d, cx));
    let edition = fields.require(field::EDITION).and_then(|d| int(d, cx));
    let toolchain = fields.get(field::TOOLCHAIN).map_or(Some(None), |d| Version::read(d, cx).map(Some));
    let include = fields.require(field::INCLUDE).and_then(|d| strings(d, cx));
    let deps = fields.get(field::DEPS).map_or(Some(Vec::new()), |d| deps(d, cx));
    let members = fields.get(field::MEMBERS).map_or(Some(Vec::new()), |d| strings(d, cx));
    let prelude_decl = fields.get(field::PRELUDE);
    let prelude = prelude_decl.map_or(Some(vec![DEFAULT_PRELUDE.to_owned()]), |d| strings(d, cx));
    let prelude_span = prelude_decl.unwrap_or(package).span;
    fields.finish(cx);
    Some(Manifest {
        name: name?,
        id: id?,
        version: version?,
        edition: edition?,
        toolchain: toolchain?,
        include: include?,
        deps: deps?,
        members: members?,
        prelude: prelude?,
        prelude_span,
    })
}

fn deps(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Vec<Dep>> {
    let Literal::Record(entries) = &decl.value else {
        return Fields::new(decl, &[], cx).map(|_| Vec::new());
    };
    let deps: Vec<_> = entries.iter().map(|entry| dep(entry, cx)).collect();
    deps.into_iter().collect()
}

fn dep(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Dep> {
    let mut fields = Fields::new(decl, field::DEP, cx)?;
    let kind = fields.get(field::KIND).map_or(Some(DepKind::Overt), |d| choice(d, DEP_KINDS, cx));
    let source = match (fields.get(field::PATH), fields.get(field::VERSION)) {
        (Some(path), None) => string(path, cx).map(Source::Path),
        (None, Some(version)) => {
            let version = Version::read(version, cx);
            let hash = fields.require(field::HASH).and_then(|d| hash(d, cx));
            let urls = fields.get(field::URLS).map_or(Some(Vec::new()), |d| strings(d, cx));
            match (version, hash, urls) {
                (Some(version), Some(hash), Some(urls)) => Some(Source::Archive { version, hash, urls }),
                _ => None,
            }
        }
        _ => {
            let msg = Msg::new().text("dependency ").code(decl.name).text(" needs exactly one of ");
            let msg =
                msg.code(format!(".{}", field::PATH)).text(" and ").code(format!(".{}", field::VERSION));
            cx.error_at(decl.span, Diagnostic::error(errors::MANIFEST_INVALID_FIELD, msg));
            None
        }
    };
    fields.finish(cx);
    Some(Dep { name: decl.name.to_owned(), source: source?, kind: kind?, span: decl.span })
}
