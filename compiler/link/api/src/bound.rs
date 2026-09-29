//! The symbols a linker defines where a program reads how its memory is laid out: the edges of
//! each placed section, the address its contents load from, and the end of each region (spec 08,
//! section 2). `.data` has `__data_start`, `__data_end` and `__data_load`; the region `ram` ends at
//! `__ram_end`.

use vun_intern::Symbol;

use super::{MemoryRegion, SectionPlacement};

/// Where in a placed section a symbol lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Start,
    End,
    /// The address its initial contents are loaded from.
    Load,
}

impl Bound {
    const fn suffix(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Load => "load",
        }
    }
}

/// What starts every bound's symbol, and separates its parts.
const PREFIX: &str = "__";
const SEPARATOR: &str = "_";
/// What starts a section's name, which a build names without it.
const SECTION: &str = ".";

impl SectionPlacement {
    /// The section a build names `name`: `data` is `.data`.
    #[must_use]
    pub fn section(name: Symbol) -> Symbol {
        Symbol::intern(&[SECTION, name.as_str()].concat())
    }

    /// The symbol the linker defines at `bound` of this section.
    #[must_use]
    pub fn bound(&self, bound: Bound) -> Symbol {
        let name = self.name.as_str().trim_start_matches(SECTION);
        Symbol::intern(&[PREFIX, name, SEPARATOR, bound.suffix()].concat())
    }
}

impl MemoryRegion {
    /// The symbol the linker defines at the end of this region, where a stack growing down starts.
    #[must_use]
    pub fn end(&self) -> Symbol {
        Symbol::intern(&[PREFIX, self.name.as_str(), SEPARATOR, Bound::End.suffix()].concat())
    }
}
