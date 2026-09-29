//! What a processor needs to have an instruction beyond what every one of its instruction set
//! has: what its form requires, and what the width it is written at requires besides, each a
//! value of the table's `Need`, a feature (`lang.target.Spec.features`) or a floating-point unit
//! computing floats of a width (`lang.target.Spec.floats`).

use vun_inline::{Inst, Need, Needed, SizeId};
use vun_items::Item;

use super::Arch;
use super::table::Table;
use crate::target::Members;
use crate::ty::Value;

/// How a table's forms say what a processor needs: the field of a form holding a `?Need`, the
/// fields of its widths where they say so too, and the variants of `Need`.
pub(super) struct Needs {
    pub(super) form: Item,
    pub(super) sized: Option<Sized>,
    pub(super) feature: Item,
    pub(super) float: Option<Item>,
}

/// The fields of a form listing its widths, and of each width naming it and holding a `?Need`.
pub(super) struct Sized {
    pub(super) sizes: Item,
    pub(super) size: Item,
    pub(super) requires: Item,
}

impl Needs {
    /// What the `?Need` in `field` of `value` names, if anything.
    fn read(&self, table: &Table, value: &Value, field: Item) -> Option<Needed> {
        let Value::Variant { name, payload, .. } = Members::of(value).ok()?.option(field).ok()?? else {
            return None;
        };
        let Some(Value::Variant { name: of, .. }) = payload.first() else { return None };
        if *name == self.feature.symbol() {
            Some(Needed::Feature(*of))
        } else if self.float.is_some_and(|float| *name == float.symbol()) {
            Some(Needed::Float(table.bits(table.size(*of)?)?))
        } else {
            None
        }
    }

    /// What `form` needs at the width `size` besides what it needs at every one.
    fn sized(&self, table: &Table, form: &Value, size: SizeId) -> Option<Needed> {
        let Sized { sizes, size: field, requires } = self.sized.as_ref()?;
        let width = *table.sizes.get(usize::from(size.0))?;
        let entries = Members::of(form).ok()?.get(*sizes).ok()?.clone().items()?;
        let entry = entries.into_iter().find(|entry| {
            Members::of(entry).ok().and_then(|m| m.case(*field).ok()).is_some_and(|name| name == width)
        })?;
        self.read(table, &entry, *requires)
    }
}

impl Table {
    /// What the instructions of `insts`, of `arch`, need of a processor beyond what every one
    /// has, each once, with the first instruction needing it.
    #[must_use]
    pub fn needs<V, P, S>(&self, arch: Arch, insts: &[Inst<V, P, S>]) -> Vec<Need> {
        let Some(shape) = &arch.shape().needs else { return Vec::new() };
        let mut needs: Vec<Need> = Vec::new();
        for inst in insts {
            let op = self.ops.get(usize::from(inst.op.0));
            let Some(form) = op.and_then(|op| op.forms.get(usize::from(inst.form.0))) else { continue };
            let needed = [shape.read(self, form, shape.form), shape.sized(self, form, inst.size)];
            for needed in needed.into_iter().flatten() {
                if needs.iter().all(|n| n.needed != needed) {
                    needs.push(Need { needed, at: inst.origin });
                }
            }
        }
        needs
    }
}
