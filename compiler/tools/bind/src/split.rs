//! Binding each directory of headers in a file of its own: a binding of GTK is the files of
//! `gtk`, `gdk`, `glib` and the rest, that spread into one namespace, rather than twenty thousand
//! lines. A declaration names one of another file by its whole path, which needs no import and
//! allows headers that include each other.

use vun_binder::Part;
use vun_clang::Cursor;
use vun_syntax::make::{Expr, File};

use crate::{Binder, paths};

/// The group of a declaration none of whose file is known.
const UNPLACED: &str = "other";

impl Binder<'_> {
    /// The root file of the bindings, and the files it spreads when they are split.
    pub(crate) fn bindings(&mut self, header: &str) -> (File, Vec<Part>) {
        self.params_reserved = self.top_names();
        let Some(module) = self.module.clone() else {
            return (self.file(self.decls.clone(), Self::doc("header", header)), Vec::new());
        };
        let decls = self.decls.clone();
        let mut groups: Vec<String> = Vec::new();
        for decl in &decls {
            let group = Self::group(decl);
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        self.homes = decls.iter().filter_map(|d| Some((self.bound(d)?, Self::group(d)))).collect();
        let mut parts = Vec::new();
        for group in &groups {
            let mine: Vec<Cursor<'_>> = decls.iter().filter(|d| Self::group(d) == *group).copied().collect();
            self.part = Some(group.clone());
            let doc = Self::doc("headers in", group);
            parts.push(Part { name: group.clone(), source: self.file(mine, doc).source() });
        }
        self.part = None;
        let spreads = groups.iter().map(|g| [module.clone(), vec![g.clone()]].concat()).collect();
        (File { doc: Self::doc("header", header), spreads, items: Vec::new() }, parts)
    }

    fn doc(what: &str, name: &str) -> Vec<String> {
        vec![
            ["Bindings to the C ", what, " `", name, "`, written by `vun util ffi bind`: regenerate them"]
                .concat(),
            "rather than edit them.".to_owned(),
        ]
    }

    /// The name of the file `decl` is bound in: that of the directory it is declared in, as a name.
    fn group(decl: &Cursor<'_>) -> String {
        let dir = decl.location().file.map(|f| paths::real(&f)).and_then(|f| {
            f.parent().and_then(std::path::Path::file_name).map(|n| n.to_string_lossy().into_owned())
        });
        let name: String =
            dir.unwrap_or_default().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        match name.chars().next() {
            None => UNPLACED.to_owned(),
            Some(c) if c.is_ascii_digit() => ["_", &name].concat(),
            Some(_) => name,
        }
    }

    /// The type bound as `name`, as the file being written spells it.
    pub(crate) fn reference(&self, name: &str) -> Expr {
        match (&self.module, self.homes.get(name)) {
            (Some(module), Some(home)) if self.part.as_ref() != Some(home) => {
                Expr::Path([module.clone(), vec![home.clone(), name.to_owned()]].concat())
            }
            _ => Expr::local(name),
        }
    }
}
