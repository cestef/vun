//! Visibility in a file (spec 01, section 7): each `#[visible(...)]` must apply where it is
//! written, and a spread may select only names that reach it.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::db::NamespaceAt;
use vun_hir::tree::{ExprId, Hir, Member, Name, Spread};
use vun_intern::Symbol;

use super::Checker;
use crate::env::Env;
use crate::item::{Declared, Site};
use crate::visible::Misplaced;

impl Checker<'_, '_, '_> {
    /// Checks the member `m` of the namespace innermost in `self.homes`.
    pub(super) fn visibility(&mut self, hir: &Hir, m: &Member) {
        let Some(home) = self.homes.last().cloned() else { return };
        // A declaration's attributes apply to each name it binds.
        let (names, attrs): (Vec<Symbol>, _) = match m {
            Member::Decl(d) => {
                let mut binders = Vec::new();
                hir.binders(hir.decls[*d].pat, &mut binders);
                (binders.iter().map(|&b| hir.binders[b].name).collect(), &hir.decls[*d].attrs)
            }
            Member::Field(field) => match field.name {
                Name::Plain(name) => (vec![name], &field.attrs),
                _ => return,
            },
            Member::Spread(spread) => return self.selected(hir, spread),
        };
        for name in names {
            if let (_, Some((attr, misplaced))) = self.env.visible(name, attrs, home.clone()) {
                self.misplaced(hir, attr, name, misplaced);
            }
        }
    }

    fn misplaced(&mut self, hir: &Hir, attr: ExprId, name: Symbol, misplaced: Misplaced) {
        let (msg, help) = match misplaced {
            Misplaced::Public => (
                Msg::new().code(name.as_str()).text(" is public, so no scope widens it"),
                Msg::new().text("start the name with ").code("_").text(" to restrict it"),
            ),
            Misplaced::NotEnclosing => (
                Msg::new().text("the scope does not enclose ").code(name.as_str()),
                Msg::new().text("name a namespace around the declaration"),
            ),
        };
        let d = Diagnostic::error(errors::NAMES_VISIBILITY, msg).at(hir.exprs[attr].origin, None);
        self.diag.error(d.child(ChildKind::Help, help));
    }

    /// Reports the selected names of `spread` restricted to scopes the spread is outside of.
    fn selected(&mut self, hir: &Hir, spread: &Spread) {
        let (Some(select), Some((_, path))) = (&spread.select, hir.path(spread.path)) else { return };
        let Ok(target) = self.env.cx.get::<NamespaceAt>(&(self.env.file.package, path)) else { return };
        for &b in select {
            let name = hir.binders[b].name;
            let Some(Site::Decl(decl)) = self.env.cx.get::<Declared>(&(target.clone(), name)) else {
                continue;
            };
            let Some(visible) =
                Env::of(self.env.cx, &decl.file).map(|env| env.decl_visible(decl.decl.id, name))
            else {
                continue;
            };
            if self.env.sees(&visible, spread.path) == Some(false) {
                self.diag.error(self.env.not_visible(hir.binders[b].origin, name, &visible));
            }
        }
    }
}
