//! `vun doc [<packages>...]`: the API documentation of packages (spec 12, section 7), a page per
//! namespace and the JSON beside them, written under the first package's work directory unless
//! `--output` names another.

use std::io;
use std::path::PathBuf;

use vun_command::{Args, Arity, Command, Example, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_doc::Docs;
use vun_fs::{Edit, Tree, WORK};
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use vun_driver::Session;

const PACKAGES: Param = Param::positional("packages")
    .arity(Arity::Many)
    .about("the package directories to document; the package around this directory, or each of its workspace, otherwise")
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
        let Some(subject) = session.subject(None)? else { return Ok(session.outcome()) };
        packages = subject.packages();
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
