//! Linker scripts as data (GNU ld's command language): the memory regions of a link plan, the
//! output sections placed in them, the symbols at their bounds (`vun_link::Bound`), and the entry
//! symbol, printed once by `print`.

mod print;

use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_link::{Bound, LinkPlan, LinkerInfo};

/// A linker script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub entry: Option<Symbol>,
    pub memory: Vec<Region>,
    pub sections: Vec<OutputSection>,
}

/// `name : ORIGIN = origin, LENGTH = length` in `MEMORY`, and the symbol at its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub name: Symbol,
    pub origin: u64,
    pub length: u64,
    pub end: Symbol,
}

/// An output section: the input sections it gathers, the region holding it at run time, and the
/// region its contents load from, when that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSection {
    pub name: Symbol,
    pub inputs: Inputs,
    pub region: Symbol,
    pub load: Option<Loaded>,
    /// The symbols at its start and end.
    pub start: Symbol,
    pub end: Symbol,
}

/// The region a section's contents load from, and the symbol at that address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub region: Symbol,
    pub at: Symbol,
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

/// Why a plan has no linker script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unscripted {
    /// A name the command language does not read as one: it would end a clause or start another.
    Name(Symbol),
    /// A section placed in a region the plan does not declare.
    Region { section: Symbol, region: Symbol },
}

impl Unscripted {
    /// The report that the linker `info` cannot link the plan.
    #[must_use]
    pub fn report(self, info: &LinkerInfo) -> Diagnostic {
        let msg = Msg::new().text("linker ").code(info.id.0).text(" cannot place ");
        let msg = match self {
            Self::Name(name) => msg.code(name.as_str()).text(": its script does not read it as a name"),
            Self::Region { section, region } => msg
                .code(section.as_str())
                .text(" in ")
                .code(region.as_str())
                .text(", which no region declares"),
        };
        Diagnostic::error(errors::LINK_UNSUPPORTED, msg)
    }
}

impl Script {
    /// The script placing what `plan` places: each section gathers its own name and those under
    /// it.
    ///
    /// # Errors
    /// Returns what the script could not say: a name ld would not read as one, or a region no
    /// declaration names.
    pub fn of(plan: &LinkPlan) -> Result<Self, Unscripted> {
        let memory = plan
            .regions
            .iter()
            .map(|r| {
                Ok(Region {
                    name: Self::name(r.name)?,
                    origin: r.origin,
                    length: r.len,
                    end: Self::name(r.end())?,
                })
            })
            .collect::<Result<_, _>>()?;
        let declared = |section: Symbol, region: Symbol| {
            let known = plan.regions.iter().any(|r| r.name == region);
            if known { Ok(region) } else { Err(Unscripted::Region { section, region }) }
        };
        let sections = plan
            .sections
            .iter()
            .map(|s| {
                let name = Self::name(s.name)?;
                Ok(OutputSection {
                    name,
                    inputs: Inputs {
                        keep: s.keep,
                        patterns: vec![Pattern::Exact(name), Pattern::Under(name)],
                    },
                    region: declared(name, s.region)?,
                    load: s
                        .load_region
                        .map(|load| {
                            Ok(Loaded {
                                region: declared(name, load)?,
                                at: Self::name(s.bound(Bound::Load))?,
                            })
                        })
                        .transpose()?,
                    start: Self::name(s.bound(Bound::Start))?,
                    end: Self::name(s.bound(Bound::End))?,
                })
            })
            .collect::<Result<_, _>>()?;
        let entry = plan.entry.map(Self::name).transpose()?;
        Ok(Self { entry, memory, sections })
    }

    /// `name`, when ld reads it as a name unquoted: a letter, `_` or `.`, then those or digits.
    fn name(name: Symbol) -> Result<Symbol, Unscripted> {
        let word = |c: char| c.is_ascii_alphabetic() || c == '_' || c == '.';
        let mut chars = name.as_str().chars();
        let named = chars.next().is_some_and(word) && chars.all(|c| word(c) || c.is_ascii_digit());
        if named { Ok(name) } else { Err(Unscripted::Name(name)) }
    }
}
