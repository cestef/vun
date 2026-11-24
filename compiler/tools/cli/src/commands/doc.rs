//! `vun doc [<packages>...]`: the API documentation of packages (spec 12, section 7), a page per
//! namespace and the JSON beside them, written under the first package's work directory unless
//! `--output` names another.

use std::io;
use std::path::PathBuf;

use vun_build::layout::MANIFEST;
use vun_command::{Args, Arity, Command, CommandVerb, Example, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_fs::Edit;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use vun_driver::Session;

const PACKAGES: Param = Param::positional("packages")
    .arity(Arity::Many)
    .about("the package directories to document; the package around this directory, or each of its workspace, otherwise")
    .values(Values::Path(PathKind::Dir));
const DEPS: Param = Param::flag("deps").about("document the packages they declare as dependencies too");
const LIBRARY: Param = Param::flag("library").about("document the toolchain's library too");
const OUTPUT: Param = Param::option("output")
    .short('o')
    .about("the directory the site is written to")
    .values(Values::Path(PathKind::Dir));
/// The directory under the work directory the site is written to.
const DOCS: &str = "doc";
/// The documenter of Vun packages.
const DOCUMENTER: &str = "html";
/// The file that marks a directory as one `vun doc` writes in.
const STAMP: &str = ".vun-doc";

pub(super) const DOC: Command<Session> =
    Command::run(CommandVerb::Doc, "write the API documentation of packages", doc)
        .params(&[PACKAGES, DEPS, LIBRARY, OUTPUT])
        .examples(&[
            Example { line: "", about: "document the project around this directory" },
            Example { line: "--deps", about: "document the project and the packages it depends on" },
            Example {
                line: "--library -o site/public/api",
                about: "document the project and the toolchain's library, for the site to serve",
            },
        ]);

/// The directory the documentation is written to.
struct Output(PathBuf);

impl Output {
    /// Whether the documentation may be written here: the directory is absent, empty or stamped.
    fn open(&self) -> io::Result<bool> {
        match std::fs::read_dir(&self.0) {
            Ok(mut entries) => Ok(entries.next().is_none() || self.0.join(STAMP).exists()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// Writes `files` here, unless the directory is one the documentation may not be written to;
    /// whether it was.
    fn write(&self, files: Vec<vun_documenter::Written>) -> io::Result<bool> {
        if !self.open()? {
            return Ok(false);
        }
        self.0.join(STAMP).put("")?;
        for written in files {
            self.0.join(&written.path).put(written.contents)?;
        }
        Ok(true)
    }

    fn refusal(&self) -> Diagnostic {
        let msg = Msg::new()
            .code(self.0.display().to_string())
            .text(" holds files ")
            .code("vun doc")
            .text(" did not write");
        Diagnostic::error(errors::DOC_FOREIGN_OUTPUT, msg).help(
            Msg::new()
                .text("write the documentation into an empty directory, such as ")
                .code("site/public/api"),
        )
    }
}

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
    if args.flag(&DEPS) {
        let declared: Vec<PathBuf> = packages.iter().flat_map(|dir| session.declared_deps(dir)).collect();
        packages.extend(declared);
    }
    if args.flag(&LIBRARY) {
        packages.extend(session.library_packages());
    }
    packages.dedup();
    let Some(documenter) = session.registry.documenters.get(DOCUMENTER) else {
        let msg = Msg::new().text("this program runs without ").code("doc");
        session.diag.error(Diagnostic::error(errors::CLI_UNAVAILABLE, msg));
        return Ok(Outcome::Failure);
    };
    let documenter = documenter.item.clone();
    let site = match session.document(&*documenter, &packages) {
        Ok(site) => site,
        Err(e) => {
            let msg = Msg::new().text("the documenter ").code(documenter.name()).text(" failed");
            session.diag.error(Diagnostic::error(errors::COMPONENT_FAILED, msg).caused_by(&e));
            return Ok(Outcome::Failure);
        }
    };
    let work = session.work(&packages[0].join(MANIFEST));
    let dir = args.value(&OUTPUT).map_or_else(|| work.join(DOCS), PathBuf::from);
    let output = Output(dir);
    match output.write(site.files) {
        Ok(true) => {}
        Ok(false) => {
            session.diag.error(output.refusal());
            return Ok(session.outcome());
        }
        Err(e) => {
            session.diag.error(Diagnostic::unwritable(&output.0, &e));
            return Ok(session.outcome());
        }
    }
    let dir = output.0;
    shell.status(&Documented { namespaces: site.namespaces, dir });
    Ok(session.outcome())
}
