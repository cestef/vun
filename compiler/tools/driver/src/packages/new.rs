//! Starting a project from a template (`vun new`): a project of the library's `templates`
//! directory, copied, and given the name of its directory and an id of its own.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vun_build::layout::MANIFEST;
use vun_build::manifest::Manifest;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::{Edit, Tree};
use vun_hash::Hash;
use vun_suggest::closest;

use super::Library;
use crate::Session;

impl Session {
    /// Starts a project in `dir` from the template named `template`, returning the files made,
    /// relative to `dir`.
    ///
    /// # Errors
    /// Returns once what stopped it is reported.
    pub fn new_project(&mut self, dir: &Path, template: &str) -> Result<Vec<PathBuf>, ErrorGuaranteed> {
        let from = self.library.join(Library::TEMPLATES).join(template);
        if !from.is_dir() {
            let names = Library::templates(&self.library);
            let msg = Msg::new().text("no template ").code(template).text(" in the library");
            let help = closest(template, names.iter().map(String::as_str))
                .map_or_else(|| Msg::new().text("templates: ").code_list(names.iter()), Msg::did_you_mean);
            return Err(self.diag.error(Diagnostic::error(errors::NEW_NO_TEMPLATE, msg).help(help)));
        }
        let manifest = dir.join(MANIFEST);
        if manifest.exists() {
            let msg =
                Msg::new().code(crate::subject::Subject::relative(dir)).text(" already holds a package");
            let help = Msg::new().text("choose a directory without a ").code(MANIFEST);
            return Err(self.diag.error(Diagnostic::error(errors::NEW_EXISTS, msg).help(help)));
        }
        let files = from.files().map_err(|e| self.diag.error(Diagnostic::unreadable(&from, &e)))?;
        let mut made = Vec::new();
        for file in files {
            let relative = file.strip_prefix(&from).unwrap_or(&file).to_path_buf();
            let bytes =
                std::fs::read(&file).map_err(|e| self.diag.error(Diagnostic::unreadable(&file, &e)))?;
            let to = dir.join(&relative);
            to.put(bytes).map_err(|e| self.diag.error(Diagnostic::unwritable(&to, &e)))?;
            made.push(relative);
        }
        let name = std::path::absolute(dir).ok().and_then(|d| Some(d.file_name()?.to_str()?.to_owned()));
        let name = name.unwrap_or_else(|| template.to_owned());
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let id = Hash::of_value(&(&name, nanos)).short();
        let file = self.load(&manifest).ok_or_else(|| self.failure())?;
        let what = Msg::new().text("give ").code(&name).text(" its id");
        if !self
            .rewrite_manifest(file, &manifest, what, |decls, text| Manifest::identify(decls, text, &name, id))
        {
            return Err(self.failure());
        }
        Ok(made)
    }

    fn failure(&self) -> ErrorGuaranteed {
        self.diag.failed().expect("what stopped it is reported")
    }
}
