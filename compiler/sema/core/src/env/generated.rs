//! The entries of a list, those `$for` and `$if` generate included (spec 03, section 2), each
//! with the environment it is generated under.

use vun_hir::tree::{Entry, List, PatKind, Staged};
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;
use vun_sema_ty::trap::Unevaluated;

use super::Env;

impl Env<'_, '_> {
    /// Pushes onto `out` what `entry` stands for: itself when written, and when generated, each
    /// entry of the branch a `$if` takes or of every step of a `$for`, with the step's binding.
    ///
    /// # Errors
    /// Returns why a condition or the items of a `$for` are not known here.
    pub fn generated<'e, T>(
        &self,
        entry: &'e Entry<T>,
        out: &mut Vec<(Self, &'e T)>,
    ) -> Result<(), Unevaluated> {
        let staged = match entry {
            Entry::One(item) => {
                out.push((self.clone(), item));
                return Ok(());
            }
            Entry::Staged(staged) => staged,
        };
        match staged {
            Staged::If { cond, then, otherwise, .. } => match self.tested(cond)? {
                Some(bound) => self.clone().staging(&bound).list(then, out),
                None => self.list(otherwise, out),
            },
            Staged::For { pat, iter, body, .. } => {
                let hir = &self.parsed.hir;
                let items = self.eval(*iter)?.items().ok_or(Unevaluated::Unsupported)?;
                // `_` binds nothing: each step is the body as written.
                let binder = match hir.pats[*pat].kind {
                    PatKind::Bind(binder) => Some(Node { id: binder, origin: hir.binders[binder].origin }),
                    PatKind::Hole => None,
                    _ => return Err(Unevaluated::Unsupported),
                };
                for value in items {
                    let staged: Vec<Staging> =
                        binder.map(|binder| Staging { binder, value }).into_iter().collect();
                    self.clone().staging(&staged).list(body, out)?;
                }
                Ok(())
            }
        }
    }

    /// [`Env::generated`] for every entry of `list`.
    ///
    /// # Errors
    /// Returns why an entry cannot be generated here.
    pub fn list<'e, T>(&self, list: &'e List<T>, out: &mut Vec<(Self, &'e T)>) -> Result<(), Unevaluated> {
        list.iter().try_for_each(|entry| self.generated(entry, out))
    }
}
