//! Views of what the front end reads: tokens, the syntax tree, a package's manifest and plan, and
//! the high-level representation.

use std::io::{self, Write};

use vun_hir::db::Package;
use vun_span::{FileId, Span};

use crate::Session;

impl Session {
    pub(super) fn view_tokens(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let tokens = self.lex(file);
        write!(out, "{}", tokens.dump(self.text(file)))
    }

    pub(super) fn view_cst(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let tokens = self.lex(file);
        let cst = self.parse(&tokens);
        write!(out, "{}", cst.dump(&tokens, self.text(file)))
    }

    pub(super) fn view_manifest(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        match self.manifest(file) {
            Some(manifest) => writeln!(out, "{manifest:#?}"),
            None => Ok(()),
        }
    }

    pub(super) fn view_plan(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        for action in self.plan(file).map(|p| p.actions).unwrap_or_default() {
            // What no literal spells, such as a type, shows as diagnostics show values.
            let span = Span::new(file, 0, 0);
            let package = |p| self.db.get::<Package>(&p).name;
            match action.literal(span, &package).and_then(|l| l.source().ok()) {
                Some(source) => writeln!(out, "{source}")?,
                None => writeln!(out, "{action}")?,
            }
        }
        Ok(())
    }

    pub(super) fn view_hir(&mut self, file: FileId, out: &mut dyn Write) -> io::Result<()> {
        let parsed = self.parsed(file);
        self.check_names(file);
        write!(out, "{}", parsed.hir.dump(&parsed.resolution, self.origins, self.sources))
    }
}
