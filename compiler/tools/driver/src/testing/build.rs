//! The program running a namespace's tests: which namespace a path names, and building it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_hir::db::{Namespace, Package};
use vun_mir::{Program, Programs, Roots};
use vun_sema::testing::Runner;
use vun_sema_ty::trap::Unevaluated;
use vun_span::{EXTENSION, FileId};

use crate::Session;

impl Session {
    /// The namespace whose tests `path` names, in the package of `root`: a file's or a
    /// directory's of the package, its root one at the package's directory or with no path.
    /// Reported when there is none, as for a file outside any package.
    pub fn tested(&mut self, root: FileId, path: Option<&Path>) -> Option<Namespace> {
        let ns = self.namespace_at(root, path);
        if ns.is_none() {
            let msg = Msg::new()
                .code(path.map(|p| p.display().to_string()).unwrap_or_default())
                .text(" is no namespace of a package");
            let help = Msg::new().text(
                "name a file or directory of a package, or its directory; a file outside any is in none",
            );
            self.diag.error(Diagnostic::error(errors::TEST_NO_NAMESPACE, msg).help(help));
        }
        ns
    }

    /// The namespace at `path` in the package of `root`, as [`Self::tested`] finds it, unreported.
    pub(crate) fn namespace_at(&mut self, root: FileId, path: Option<&Path>) -> Option<Namespace> {
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
        names.map(|path| Namespace { package, path })
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

    /// The program [`Self::build_tests`] builds for `ns`: the functions its tests reach.
    pub fn tests_program(&mut self, root: FileId, ns: Namespace) -> Option<Arc<Program>> {
        let key = self.file_ref(root);
        let ty = self.db.get::<Runner>(&(key.clone(), ns)).ok()?;
        Some(self.db.get::<Programs>(&(key, Roots::Members(ty))))
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
