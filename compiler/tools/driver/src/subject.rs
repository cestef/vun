//! What a command's path names (decision 356): a file outside any package, a package or a part of
//! one, or a workspace. With no path, the package around the current directory, or the workspace
//! it roots, as cargo finds `Cargo.toml`.

use std::io;
use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_diag::{Diagnostic, Msg, errors};
use vun_fs::Tree;
use vun_span::EXTENSION;

use crate::Session;

/// What a path names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// A file outside any package: a program of its own.
    Loose(PathBuf),
    /// A package in `dir`, and the file or directory of it the path names; `None` for the package
    /// itself.
    Package { dir: PathBuf, within: Option<PathBuf> },
    /// The workspace rooted in `dir`: its root package and the members it lists.
    Workspace { dir: PathBuf, members: Vec<PathBuf> },
}

impl Subject {
    /// The directories of the packages it covers, the workspace's root first; none for a loose
    /// file.
    #[must_use]
    pub fn packages(&self) -> Vec<PathBuf> {
        match self {
            Self::Loose(_) => Vec::new(),
            Self::Package { dir, .. } => vec![dir.clone()],
            Self::Workspace { dir, members } => [dir].into_iter().chain(members).cloned().collect(),
        }
    }

    /// The file a command starts from: the loose file, the one named in a package, or the
    /// manifest of the package or of the workspace's root.
    #[must_use]
    pub fn root(&self) -> PathBuf {
        match self {
            Self::Loose(file) => file.clone(),
            Self::Package { within: Some(within), .. } if !within.is_dir() => within.clone(),
            Self::Package { dir, .. } | Self::Workspace { dir, .. } => dir.join(MANIFEST),
        }
    }

    /// What its tests are collected from, each a root file and the namespace the path names
    /// there: the loose file itself, the package's with what the path names, or each package's
    /// root namespace of a workspace.
    #[must_use]
    pub fn tested(&self) -> Vec<(PathBuf, Option<PathBuf>)> {
        match self {
            Self::Loose(file) => vec![(file.clone(), Some(file.clone()))],
            Self::Package { dir, within } => vec![(dir.join(MANIFEST), within.clone())],
            Self::Workspace { .. } => self.packages().iter().map(|dir| (dir.join(MANIFEST), None)).collect(),
        }
    }

    /// What the code of the program built from `root`, a root file `tested` gives, is below: the
    /// package's directory, or the loose file itself.
    #[must_use]
    pub fn scope(root: &Path) -> &Path {
        match root.parent() {
            Some(dir) if root.file_name().is_some_and(|name| name == MANIFEST) => dir,
            _ => root,
        }
    }

    /// How it is named to people when no path named it: a file by its path, below the current
    /// directory when it is there; a package or a workspace by its directory, which is its own
    /// name when it is the current one.
    #[must_use]
    pub fn shown(&self) -> String {
        match self {
            Self::Package { dir, within: None } | Self::Workspace { dir, .. } => {
                let below = Self::relative(dir);
                match (below.is_empty(), dir.file_name()) {
                    (true, Some(name)) => name.to_string_lossy().into_owned(),
                    _ => below,
                }
            }
            Self::Loose(_) | Self::Package { .. } => Self::relative(&self.root()),
        }
    }

    /// How `path` is named to people: below the current directory when it is there.
    #[must_use]
    pub fn relative(path: &Path) -> String {
        vun_span::SourceFile::below_here(path)
    }

    /// The file it names, when it names one: a loose file or a file of a package.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::Loose(file) => Some(file),
            Self::Package { within: Some(within), .. } if !within.is_dir() => Some(within),
            Self::Package { .. } | Self::Workspace { .. } => None,
        }
    }
}

impl Session {
    /// What `path` names, or with none the package around the current directory; `None` once
    /// its absence, a directory outside any package or a member holding none, is reported.
    ///
    /// # Errors
    /// Fails when the current directory cannot be read.
    pub fn subject(&mut self, path: Option<&Path>) -> io::Result<Option<Subject>> {
        if let Some(missing) = path.filter(|p| !p.exists()) {
            self.missing(missing);
            return Ok(None);
        }
        let (dir, within) = match path {
            None => match self.project()? {
                Some((dir, _)) => (dir, None),
                None => return Ok(None),
            },
            Some(path) => {
                let dir = if path.is_dir() { path } else { path.parent().unwrap_or(Path::new("")) };
                match self.package_around(dir) {
                    Some((found, _)) => {
                        let whole = path == found || path == found.join(MANIFEST);
                        (found, (!whole).then(|| path.to_path_buf()))
                    }
                    None if path.is_dir() => {
                        self.outside(path);
                        return Ok(None);
                    }
                    None => return Ok(Some(Subject::Loose(path.to_path_buf()))),
                }
            }
        };
        let Some(key) = self.package_at(&dir) else { return Ok(None) };
        let members = self.members(key);
        if within.is_some() || members.is_empty() {
            return Ok(Some(Subject::Package { dir, within }));
        }
        let missing: Vec<PathBuf> =
            members.iter().filter(|m| self.package_at(m).is_none()).cloned().collect();
        for member in &missing {
            let msg = Msg::new().text("the workspace's member ").code(member.display().to_string());
            let msg = msg.text(" holds no package");
            let help =
                Msg::new().text("a member is a directory whose ").code(MANIFEST).text(" declares a package");
            self.diag.error(Diagnostic::error(errors::PACKAGE_MISSING_MEMBER, msg).help(help));
        }
        Ok(missing.is_empty().then_some(Subject::Workspace { dir, members }))
    }

    /// The source files `subject` covers, each package's manifest first: a loose file, the files
    /// of a package under what the path names, or every file of each package of a workspace.
    pub fn files(&mut self, subject: &Subject) -> Vec<PathBuf> {
        let within = match subject {
            Subject::Loose(file) => return vec![file.clone()],
            Subject::Package { within, .. } => within.as_deref(),
            Subject::Workspace { .. } => None,
        };
        let mut files = Vec::new();
        for dir in subject.packages() {
            let Some(key) = self.package_at(&dir) else { continue };
            let manifest = dir.join(MANIFEST);
            let sources = self.packages.list[&key].files().filter(|file| *file != manifest);
            let sources = sources.filter(|file| file.extension().is_some_and(|e| e == EXTENSION));
            let sources: Vec<PathBuf> = sources.collect();
            if within.is_none() {
                files.push(manifest);
            }
            files.extend(sources.into_iter().filter(|file| within.is_none_or(|w| file.starts_with(w))));
        }
        files
    }

    /// The source files `path` names, for a command that reads source and needs no package: what
    /// [`Self::subject`] covers, and for a directory in no package every source file below it
    /// (a group of packages, a directory of loose files). `None` once why none is named is
    /// reported.
    ///
    /// # Errors
    /// Fails when the current directory or the directory named cannot be read.
    pub fn sources(&mut self, path: Option<&Path>) -> io::Result<Option<Vec<PathBuf>>> {
        if let Some(dir) = path.filter(|p| p.is_dir() && self.package_around(p).is_none()) {
            return dir.files_with(EXTENSION).map(Some);
        }
        Ok(self.subject(path)?.map(|subject| self.files(&subject)))
    }

    /// Reports that `path` names nothing, suggesting the closest name beside it.
    fn missing(&mut self, path: &Path) {
        let beside = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let names: Vec<String> = std::fs::read_dir(beside)
            .into_iter()
            .flatten()
            .filter_map(|entry| Some(entry.ok()?.file_name().to_str()?.to_owned()))
            .collect();
        let given = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        let near = vun_suggest::closest(given, names.iter().map(String::as_str));
        let msg = Msg::new().code(path.display().to_string()).text(" does not exist");
        let mut d = Diagnostic::error(errors::CLI_MISSING_PATH, msg);
        if let Some(near) = near {
            let near = if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
                beside.join(near)
            } else {
                near.into()
            };
            d = d.help(Msg::did_you_mean(near.display().to_string()));
        }
        self.diag.error(d);
    }

    /// Reports that `dir` is in no package.
    fn outside(&mut self, dir: &Path) {
        let msg = Msg::new().code(dir.display().to_string()).text(" is in no package: no ").code(MANIFEST);
        let msg = msg.text(" declaring a package in it or above");
        let help = Msg::new().text("name a package's directory, a file or directory of one, or a file");
        self.diag.error(Diagnostic::error(errors::PACKAGE_NO_PROJECT, msg).help(help));
    }
}
