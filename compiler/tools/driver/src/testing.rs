//! Running tests (spec 11): the program `std.testing.runner` gives for a namespace, built from a
//! root file as any program is, and the lines it writes, read as typed values.

use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_data::{FromData, ToData};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_hir::db::{Namespace, Package};
use vun_mir::Roots;
use vun_sema::testing::Runner;
use vun_sema_ty::trap::Unevaluated;
use vun_span::{EXTENSION, FileId};

use crate::Session;

/// A line the runner writes (`std.testing.Line`): a test run, then the totals.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Line {
    Test(Ran),
    Totals(Totals),
}

/// A test run: its name, as declared, and what became of it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Ran {
    pub name: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub enum Outcome {
    Passed,
    Trapped(Trapped),
}

/// A trap ending a test: its site, and what the program's table says of it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Trapped {
    pub site: u32,
    pub said: Option<Said>,
}

/// What the program's table says of a trap site (`std.trace.Site`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Said {
    pub message: Option<String>,
    pub at: Option<Place>,
}

/// A place in source, its line and column counted from 1 (`std.trace.Place`).
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Place {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// How many tests passed and how many trapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToData, FromData)]
pub struct Totals {
    pub passed: usize,
    pub trapped: usize,
}

impl Line {
    /// The line the runner wrote as `text`; `None` for what the tests write themselves.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        vun_data::json::parse(text).ok()?.read().ok()
    }
}

impl Session {
    /// The file the program running the tests at `path` is built from: the manifest of the
    /// package around `path`, or around the current directory with no path, or, outside any
    /// package, the file `path` itself. With no path and no package, that is reported.
    ///
    /// # Errors
    /// Fails when the current directory cannot be read.
    pub fn tests_root(&mut self, path: Option<&Path>) -> std::io::Result<Option<PathBuf>> {
        let Some(path) = path else { return Ok(self.project()?.map(|(dir, _)| dir.join(MANIFEST))) };
        let dir = if path.is_dir() { path } else { path.parent().unwrap_or(Path::new("")) };
        Ok(Some(self.package_around(dir).map_or_else(|| path.to_path_buf(), |(dir, _)| dir.join(MANIFEST))))
    }

    /// The namespace whose tests `path` names, in the package of `root`: a file's or a
    /// directory's of the package, its root one at the package's directory or with no path.
    /// Reported when there is none, as for a file outside any package.
    pub fn tested(&mut self, root: FileId, path: Option<&Path>) -> Option<Namespace> {
        let key = self.package_of(root);
        let package = self.publish(key);
        let (name, relative) = {
            let own = &self.packages.list[&key];
            (own.name, path.map(|p| own.relative(p)).unwrap_or_default())
        };
        let data = self.db.get::<Package>(&package);
        // A directory's namespace is the one of the file beside it (spec 01, section 8).
        let file = Path::new(&relative).with_extension(EXTENSION);
        let names = match name {
            Some(_) if relative.is_empty() => Some(Vec::new()),
            Some(_) => data.namespace_of(&file.display().to_string()).filter(|names| data.holds(names)),
            None => None,
        };
        let Some(names) = names else {
            let msg = Msg::new()
                .code(path.map(|p| p.display().to_string()).unwrap_or_default())
                .text(" is no namespace of a package");
            let help = Msg::new().text(
                "name a file or directory of a package, or its directory; a file outside any is in none",
            );
            self.diag.error(Diagnostic::error(errors::TEST_NO_NAMESPACE, msg).help(help));
            return None;
        };
        Some(Namespace { package, path: names })
    }

    /// Builds the program running the tests of `ns` from `root`: the exports among the members
    /// of the type `std.testing.runner` gives for it.
    ///
    /// # Errors
    /// Returns once what stopped the build is reported.
    pub fn build_tests(
        &mut self,
        root: FileId,
        ns: Namespace,
        output: Option<&Path>,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        let key = self.file_ref(root);
        let ty = match self.db.get::<Runner>(&(key, ns)) {
            Ok(ty) => ty,
            Err(why) => return Err(self.unrunnable(root, &why)),
        };
        let asked = std::mem::replace(&mut self.roots, Roots::Members(ty));
        let built = self.build(root, output);
        self.roots = asked;
        built
    }

    /// Reports why the runner gave no program, at the root namespace of `root`.
    fn unrunnable(&mut self, root: FileId, why: &Unevaluated) -> ErrorGuaranteed {
        let parsed = self.parsed(root);
        let at = parsed.hir.root.map(|ns| parsed.hir.namespaces[ns].origin);
        if let (Unevaluated::Trapped(trap), Some(at)) = (why, at) {
            return self.diag.error(trap.report(at));
        }
        let msg = Msg::new().text("the tests could not be collected: ").text(why.why());
        let d = Diagnostic::error(errors::TEST_UNRUNNABLE, msg);
        self.diag.error(match at {
            Some(at) => d.at(at, None),
            None => d,
        })
    }
}
