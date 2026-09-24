//! Linker scripts as data (GNU ld's command language): the memory regions of a link plan, the
//! output sections placed in them, and the entry symbol, printed once by `print`.

mod print;

use vun_intern::Symbol;
use vun_link::LinkPlan;

/// A linker script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub entry: Option<Symbol>,
    pub memory: Vec<Region>,
    pub sections: Vec<OutputSection>,
}

/// `name : ORIGIN = origin, LENGTH = length` in `MEMORY`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub name: Symbol,
    pub origin: u64,
    pub length: u64,
}

/// An output section: the input sections it gathers, the region holding it at run time, and the
/// region its contents load from, when that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSection {
    pub name: Symbol,
    pub inputs: Inputs,
    pub region: Symbol,
    pub load: Option<Symbol>,
}

/// Input sections of every input file, by pattern; kept even when unreferenced where `keep`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub keep: bool,
    pub patterns: Vec<Pattern>,
}

/// A pattern naming input sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// The section of this name.
    Exact(Symbol),
    /// Every section named this, a dot, then anything: `.text.*`, where compilers put each
    /// function's code.
    Under(Symbol),
}

impl Script {
    /// The script placing what `plan` places: each section gathers its own name and those under
    /// it.
    #[must_use]
    pub fn of(plan: &LinkPlan) -> Self {
        let memory =
            plan.regions.iter().map(|r| Region { name: r.name, origin: r.origin, length: r.len }).collect();
        let sections = plan
            .sections
            .iter()
            .map(|s| OutputSection {
                name: s.name,
                inputs: Inputs {
                    keep: s.keep,
                    patterns: vec![Pattern::Exact(s.name), Pattern::Under(s.name)],
                },
                region: s.region,
                load: s.load_region,
            })
            .collect();
        Self { entry: plan.entry, memory, sections }
    }
}
