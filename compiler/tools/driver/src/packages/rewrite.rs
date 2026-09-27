//! Writing a project's manifest back after edits computed from its literal declarations, so that
//! everything a command changes goes through the literal writer and keeps the file's layout.

use std::error::Error;
use std::path::Path;

use vun_build::manifest::PACKAGE;
use vun_diag::{Diagnostic, Edit, Msg, errors};
use vun_fs::Edit as _;
use vun_span::{FileId, Span};
use vun_syntax::literal::{Decl, Unwritable};

use crate::Session;

impl Session {
    /// Applies the edits `edit` computes from the declarations and text of the manifest loaded as
    /// `file` from `path`, and writes it back; `what` names the change when it cannot be made,
    /// which is reported. Returns whether the manifest was written.
    pub(super) fn rewrite_manifest(
        &mut self,
        file: FileId,
        path: &Path,
        what: Msg,
        edit: impl FnOnce(&[Decl<'_>], &str) -> Result<Vec<Edit>, Unwritable>,
    ) -> bool {
        let text = self.text(file).to_owned();
        let (edits, package) = self.declarations(file, |decls, _| {
            let package = decls.iter().find(|d| d.name == PACKAGE).map(|d| d.span);
            (edit(decls, &text), package)
        });
        let rewritten = match edits {
            Ok(edits) => Edit::apply(&edits, &text).map_err(|e| Box::new(e) as Box<dyn Error>),
            Err(e) => Err(Box::new(e) as Box<dyn Error>),
        };
        let rewritten = match rewritten {
            Ok(rewritten) => rewritten,
            Err(e) => {
                let msg = Msg::new().text("cannot ").append(what).text(" in this manifest");
                let d = Diagnostic::error(errors::MANIFEST_UNEDITABLE, msg).caused_by(e.as_ref());
                self.report(Some(package.unwrap_or_else(|| Span::new(file, 0, 0))), d);
                return false;
            }
        };
        if let Err(e) = path.put(rewritten) {
            self.diag.error(Diagnostic::unwritable(path, &e));
            return false;
        }
        self.packages.wrote(path);
        true
    }
}
