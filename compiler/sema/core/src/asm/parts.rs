//! What a call writes before its operands (spec 07, section 6): the types a form computes or the
//! width it accesses (`vcvt(i32, f32, ...)`, `ldr(u8, ...)`), and the width of its encoding
//! (`add(w, ...)`); and the forms of an instruction they select, in order, each with the width it
//! fixes. A width listing types is selected by a call writing them, one listing none by a call
//! writing none or the one type of its width (`ldr(w1, ...)` and `ldr(u32, w1, ...)`, not `ldrb`).

use vun_inline::{FormId, OpId, SizeId};
use vun_intern::Symbol;
use vun_isa::Fixed;
use vun_items::Item;
use vun_sema_ty::{Ty, Value};

use super::table::Table;
use crate::target::Members;

/// How a table's forms list their widths.
pub(super) struct Widths {
    /// The field of a form listing the widths it is defined at.
    pub(super) of: Item,
    /// The field of each listed entry naming its width, where the entry is not the width itself.
    pub(super) size: Option<Item>,
    /// The field of each listed entry holding the types a call writes to select it, where the
    /// table lists any.
    pub(super) types: Option<Item>,
    /// The enum of a form's widths of encoding, where a call may name one, and which of them a
    /// form is.
    pub(super) encoding: Option<(Item, Encoded)>,
}

/// The width of encoding of a form, a variant of the table's enum of them.
pub(super) type Encoded = fn(&Value) -> Option<Item>;

/// The parts a call writes: types, and a width of encoding.
#[derive(Default)]
pub(super) struct Parts {
    pub(super) types: Vec<Ty>,
    pub(super) encoding: Option<Symbol>,
}

/// A width a form is defined at, and the types selecting it.
struct Entry {
    size: Option<SizeId>,
    types: Vec<Ty>,
}

impl Table {
    /// The forms of `op` that `parts` select, in order, each with the width they fix; `width_of`
    /// gives the width of a type.
    pub(super) fn selected(
        &self,
        widths: &Widths,
        op: OpId,
        parts: &Parts,
        width_of: impl Fn(Ty) -> Option<SizeId>,
    ) -> Vec<(FormId, Fixed)> {
        let Some(forms) = self.ops.get(usize::from(op.0)).map(|o| &o.forms) else { return Vec::new() };
        let mut selected = Vec::new();
        for (at, form) in forms.iter().enumerate() {
            let Ok(at) = u16::try_from(at).map(FormId) else { continue };
            let encoding = widths.encoding.and_then(|(_, of)| of(form)).map(Item::symbol);
            if parts.encoding.is_some_and(|given| encoding != Some(given)) {
                continue;
            }
            let entries = self.entries(widths, form);
            if parts.types.is_empty() && entries.is_empty() {
                selected.push((at, Fixed::default()));
            }
            for entry in entries {
                let fits = match (entry.types.as_slice(), parts.types.as_slice()) {
                    ([], [ty]) => width_of(*ty).is_some_and(|w| Some(w) == entry.size),
                    (listed, given) => listed == given,
                };
                let fixed = Fixed { cond: None, size: entry.size.filter(|_| !parts.types.is_empty()) };
                if fits && !selected.contains(&(at, fixed)) {
                    selected.push((at, fixed));
                }
            }
        }
        selected
    }

    /// The widths `form` is defined at, as `widths` reads them.
    fn entries(&self, widths: &Widths, form: &Value) -> Vec<Entry> {
        let field = |value: &Value, item: Item| Members::of(value).ok()?.get(item).ok().cloned();
        let listed = field(form, widths.of).and_then(Value::items).unwrap_or_default();
        listed
            .iter()
            .map(|entry| {
                let size = match widths.size {
                    Some(item) => field(entry, item),
                    None => Some(entry.clone()),
                };
                let size = match size {
                    Some(Value::Variant { name, .. }) => self.size(name),
                    _ => None,
                };
                let types = widths.types.and_then(|item| field(entry, item)).and_then(Value::items);
                let types = types.unwrap_or_default().iter().filter_map(Value::as_type).collect();
                Entry { size, types }
            })
            .collect()
    }
}
