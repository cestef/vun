//! What the linker defines itself, as `wasm-ld` does: the stack pointer and the indirect function
//! table every object imports, the function calling the initialization functions, the edges of
//! the stack and the data, and the bounds of each section a source names, in the tool
//! conventions' spelling (`__start_registry`, `__stop_registry`) and the link plan's
//! (`__registry_start`, `__registry_end`, spec 13, section 7).

use vun_link::Bound;
use vun_wasm::Symbolic;

pub(crate) const STACK_POINTER: &str = "__stack_pointer";
pub(crate) const TABLE: &str = "__indirect_function_table";
pub(crate) const CALL_CTORS: &str = "__wasm_call_ctors";

/// The data symbols of fixed names, by name.
const DATA: [(&str, Provided<'static>); 5] = [
    ("__data_end", Provided::DataEnd),
    ("__heap_base", Provided::HeapBase),
    ("__dso_handle", Provided::DsoHandle),
    ("__stack_low", Provided::StackLow),
    ("__stack_high", Provided::StackHigh),
];

/// The tool conventions' names of a section's bounds start with these, then its name.
const START: &str = "__start_";
const STOP: &str = "__stop_";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Provided<'o> {
    StackPointer,
    Table,
    CallCtors,
    /// The end of the data, where nothing follows.
    DataEnd,
    /// Where a heap may start: the end of the data, aligned for anything.
    HeapBase,
    /// The address the program itself is at, null.
    DsoHandle,
    StackLow,
    StackHigh,
    /// A bound of the section a source names.
    Bound {
        section: &'o str,
        end: bool,
    },
}

impl<'o> Provided<'o> {
    /// What the linker defines as `name`, a symbol of the kind `what`, among the sections
    /// `sections` the objects name.
    pub(crate) fn named(name: &str, what: Symbolic, sections: &[&'o str]) -> Option<Self> {
        match (what, name) {
            (Symbolic::Global(_), STACK_POINTER) => Some(Self::StackPointer),
            (Symbolic::Table(_), TABLE) => Some(Self::Table),
            (Symbolic::Function(_), CALL_CTORS) => Some(Self::CallCtors),
            (Symbolic::Data(_), _) => {
                DATA.iter().find(|(fixed, _)| *fixed == name).map(|&(_, provided)| provided).or_else(|| {
                    sections.iter().find_map(|&section| {
                        let bounds = [
                            ([START, section].concat(), false),
                            ([STOP, section].concat(), true),
                            (Bound::Start.of(section).as_str().to_owned(), false),
                            (Bound::End.of(section).as_str().to_owned(), true),
                        ];
                        bounds
                            .into_iter()
                            .find(|(bound, _)| bound == name)
                            .map(|(_, end)| Self::Bound { section, end })
                    })
                })
            }
            _ => None,
        }
    }
}
