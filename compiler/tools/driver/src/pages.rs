//! The pages explaining diagnostics (spec 10, section 1): each error of `lang.errors` has a file
//! of the library, and each lint a file is checked with by default its `doc`.

use std::path::PathBuf;
use std::sync::Arc;

use vun_diag::{DiagId, errors};
use vun_sema::lint::{Checked, CheckedLints};

use crate::Session;

/// Where a page is.
pub enum Page {
    /// A file of the library.
    File(PathBuf),
    /// A lint's `doc`.
    Text(String),
}

impl Session {
    /// Every diagnostic that has a page, with it: the errors, then the lints of the packages a
    /// loose file depends on.
    pub fn pages(&mut self) -> Vec<(DiagId, Page)> {
        let lints = self.lints();
        let errors = errors::EXPLANATIONS.iter().map(|(id, page)| (*id, Page::File(self.library.join(page))));
        let pages = lints.iter().filter_map(|(id, doc)| Some((*id, Page::Text(doc.clone()?))));
        errors.chain(pages).collect()
    }

    /// The lints a loose file is checked with, each with its `doc` if it has one.
    pub fn lints(&mut self) -> Arc<Vec<(DiagId, Option<String>)>> {
        Arc::new(self.checked_lints().lints.clone())
    }

    /// The groups the lints a loose file is checked with name, by id: `std.lints.naming`.
    pub fn lint_groups(&mut self) -> Arc<Vec<DiagId>> {
        Arc::new(self.checked_lints().groups.clone())
    }

    /// The lints and groups a loose file is checked with.
    fn checked_lints(&mut self) -> Arc<Checked> {
        let loose = self.loose();
        let package = self.publish(loose);
        self.db.get::<CheckedLints>(&package)
    }
}
