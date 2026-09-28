//! `vun doc [<packages>...]`: the API documentation of packages (spec 12, section 7), a page per
//! namespace and the JSON beside them, written under the first package's work directory unless
//! `--output` names another.

use std::io;
use std::path::{Path, PathBuf};

use vun_build::layout::{Location, MANIFEST};
use vun_command::{Args, Arity, Command, Example, Outcome, Param, PathKind, Values};
use vun_diag::Diagnostic;
use vun_doc::Docs;
use vun_fs::{Edit, Tree, WORK};
use vun_intern::Symbol;
use vun_json::Object;
use vun_sema::decl::{Declaration, Declares};
use vun_span::EXTENSION;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use crate::Session;

const PACKAGES: Param = Param::positional("packages")
    .arity(Arity::Many)
    .about("the package directories to document; the project around this directory otherwise")
    .values(Values::Path(PathKind::Dir));
const LIBRARY: Param = Param::flag("library").about("document the toolchain's library too");
const OUTPUT: Param = Param::option("output")
    .short('o')
    .about("the directory the site is written to")
    .values(Values::Path(PathKind::Dir));
/// The directory under the work directory the site is written to.
const DOCS: &str = "doc";

pub(super) const DOC: Command<Session> = Command::run("doc", "write the API documentation of packages", doc)
    .params(&[PACKAGES, LIBRARY, OUTPUT])
    .examples(&[
        Example { line: "", about: "document the project around this directory" },
        Example { line: "--library -o site", about: "document the project and the toolchain's library" },
    ]);

/// Where the documentation was written.
struct Documented {
    namespaces: usize,
    dir: PathBuf,
}

impl Record for Documented {
    fn kind(&self) -> &'static str {
        "documented"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.dir.display().to_string())
            .plain(format!(", {} namespaces", self.namespaces))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("dir", self.dir.display().to_string()).with("namespaces", self.namespaces)
    }
}

impl Status for Documented {
    fn verb(&self) -> Verb {
        Verb::good("Documented")
    }
}

fn doc(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let mut packages: Vec<PathBuf> = args.values(&PACKAGES).map(PathBuf::from).collect();
    if packages.is_empty() {
        let Some((dir, _)) = session.project()? else { return Ok(session.outcome()) };
        packages.push(dir);
    }
    if args.flag(&LIBRARY) {
        packages.extend(session.library.entries()?.into_iter().filter(|dir| Session::is_package_root(dir)));
    }
    let mut docs = Docs::default();
    for package in &packages {
        session.document(&mut docs, package)?;
    }
    let dir = args.value(&OUTPUT).map_or_else(|| packages[0].join(WORK).join(DOCS), PathBuf::from);
    for written in docs.site() {
        dir.join(&written.path).put(written.contents)?;
    }
    shell.status(&Documented { namespaces: docs.namespaces.len(), dir });
    Ok(session.outcome())
}

impl Session {
    /// Whether `dir` is a package of the library: it holds its root namespace's file.
    fn is_package_root(dir: &Path) -> bool {
        dir.file_name().is_some_and(|name| dir.join(name).with_extension(EXTENSION).is_file())
    }

    /// Adds the namespaces of the package in `dir` to `docs`: each source file but the manifest,
    /// its declarations of types told from its values by their types.
    ///
    /// # Errors
    /// Fails when the directory cannot be walked.
    pub fn document(&mut self, docs: &mut Docs, dir: &Path) -> io::Result<()> {
        for file in dir.files_with(EXTENSION)? {
            let Some(path) = Location::path_of(dir, &file) else { continue };
            if file.parent() == Some(dir) && file.file_name().is_some_and(|name| name == MANIFEST) {
                continue;
            }
            match std::fs::read_to_string(&file) {
                Ok(text) => {
                    docs.add(path.clone(), file.display().to_string(), &text);
                    let loaded = self.open(&file, text);
                    let key = self.file_ref(loaded);
                    let Some(namespace) = docs.namespaces.get_mut(&path) else { continue };
                    namespace.retype(|name| {
                        let declares = self.db.get::<Declaration>(&(key.clone(), Symbol::intern(name)));
                        declares == Some(Declares::Type)
                    });
                }
                Err(e) => {
                    self.diag.error(Diagnostic::unreadable(&file, &e));
                }
            }
        }
        Ok(())
    }
}
