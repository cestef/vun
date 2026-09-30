//! A program: the objects' symbols resolved, what it keeps from its entry, its exports and the
//! symbols objects keep, and where all of it goes.

use vun_hash::Map;
use vun_wasm::Object;

use crate::layout::{Layout, Value};
use crate::live::Live;
use crate::resolve::{Resolved, Symbols};
use crate::unlinked::Unlinked;

pub(crate) struct Program<'o> {
    pub(crate) symbols: Symbols<'o>,
    pub(crate) layout: Layout<'o>,
    /// The functions exported, by name: the entry first, then the others by name.
    pub(crate) exports: Vec<(&'o str, u32)>,
}

/// The indirect function table, filled as relocations take functions' addresses.
#[derive(Default)]
pub(crate) struct Table {
    /// Its functions from its second entry on, its first being null.
    pub(crate) funcs: Vec<u32>,
    slots: Map<u32, u32>,
}

impl<'o> Program<'o> {
    /// The program `objects` make, entered at `entry` if it has one.
    ///
    /// # Errors
    /// Where a symbol is defined nowhere or twice, or an import has two types.
    pub(crate) fn of(objects: &'o [Object], entry: Option<&'o str>) -> Result<Self, Unlinked> {
        let symbols = Symbols::of(objects)?;
        let entered = match entry {
            Some(name) => match symbols.named(name) {
                Some(found @ Resolved::Func(_)) => Some((name, found)),
                _ => return Err(Unlinked::Undefined(name.to_owned())),
            },
            None => None,
        };
        let mut exported: Vec<(&str, Resolved<'o>)> = symbols
            .exported()
            .filter(|&(name, found)| matches!(found, Resolved::Func(_)) && Some(name) != entry)
            .collect();
        exported.sort_unstable_by_key(|&(name, _)| name);
        let named: Vec<(&str, Resolved<'o>)> = entered.into_iter().chain(exported).collect();
        let roots = named.iter().map(|&(_, found)| found).chain(symbols.kept());
        let live = Live::of(&symbols, roots);
        let layout = Layout::of(&symbols, &live)?;
        let exports = named
            .into_iter()
            .filter_map(|(name, found)| match layout.value(found) {
                Value::Func { index, .. } => Some((name, index)),
                _ => None,
            })
            .collect();
        Ok(Self { symbols, layout, exports })
    }
}

impl Table {
    /// The slot of the function at `index` in the table, given it the first time its address
    /// is taken.
    pub(crate) fn slot(&mut self, index: u32) -> u32 {
        *self.slots.entry(index).or_insert_with(|| {
            self.funcs.push(index);
            u32::try_from(self.funcs.len()).expect("the table fits u32")
        })
    }
}
