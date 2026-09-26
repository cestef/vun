//! The code of one function: bytes appended in order, labels, the references to labels patched
//! when the function ends, and relocations.

use std::fmt;

use vun_object::RelocKind;

use crate::Isa;
use crate::listing::Line;

/// A place in a function's code that jumps name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(u32);

impl Label {
    #[must_use]
    pub const fn number(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".L{}", self.0)
    }
}

/// An address the linker writes into the code: `target` plus `addend`, at `offset`, as `kind`
/// says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reloc<S> {
    pub offset: u64,
    pub target: S,
    pub kind: RelocKind,
    pub addend: i64,
}

/// Why a reference to a label could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpatched {
    /// The label is never bound.
    Unbound(Label),
    /// The label is further than the field reaches.
    Range { at: u64, distance: i64 },
}

impl fmt::Display for Unpatched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound(label) => write!(f, "the label {label} is never bound"),
            Self::Range { at, distance } => {
                write!(f, "the jump at {at:#x} spans {distance} bytes, further than it reaches")
            }
        }
    }
}

impl std::error::Error for Unpatched {}

/// A function's code as it is emitted.
pub struct Buffer<I: Isa, S: Copy> {
    code: Vec<u8>,
    relocs: Vec<Reloc<S>>,
    labels: Vec<Option<u64>>,
    fixups: Vec<(u64, Label, I::Fixup)>,
    /// What was emitted, when a listing is asked for.
    listing: Option<Vec<Line<I::Inst<S>>>>,
}

/// A function's finished code.
pub struct Code<T, S> {
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc<S>>,
    /// Empty unless a listing was asked for.
    pub listing: Vec<Line<T>>,
}

impl<I: Isa, S: Copy> Default for Buffer<I, S> {
    fn default() -> Self {
        Self { code: Vec::new(), relocs: Vec::new(), labels: Vec::new(), fixups: Vec::new(), listing: None }
    }
}

impl<I: Isa, S: Copy> Buffer<I, S> {
    /// A buffer that lists what is emitted into it.
    #[must_use]
    pub fn listed() -> Self {
        Self { listing: Some(Vec::new()), ..Self::default() }
    }

    #[must_use]
    pub fn here(&self) -> u64 {
        self.code.len() as u64
    }

    pub fn put(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    /// # Panics
    /// Past 2^32 labels in one function.
    #[must_use]
    pub fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(u32::try_from(self.labels.len() - 1).expect("labels fit 32 bits"))
    }

    /// Binds `label` to where the next instruction goes.
    pub fn bind(&mut self, label: Label) {
        self.labels[label.0 as usize] = Some(self.here());
        if let Some(listing) = &mut self.listing {
            listing.push(Line::Label(label));
        }
    }

    /// Records a reference to `label` at `at`, written when the function ends.
    pub fn refer(&mut self, at: u64, label: Label, fixup: I::Fixup) {
        self.fixups.push((at, label, fixup));
    }

    /// Records that the linker writes `target` plus `addend` at `offset`.
    pub fn relocate(&mut self, offset: u64, target: S, kind: RelocKind, addend: i64) {
        self.relocs.push(Reloc { offset, target, kind, addend });
    }

    /// Whether what is emitted is listed.
    #[must_use]
    pub fn lists(&self) -> bool {
        self.listing.is_some()
    }

    /// Lists `inst`, emitted from `at` to here.
    ///
    /// # Panics
    /// If the instruction spans more than 4 GiB, which none does.
    pub fn list(&mut self, at: u64, inst: I::Inst<S>) {
        let len = self.here() - at;
        if let Some(listing) = &mut self.listing {
            listing.push(Line::Inst { at, len: u32::try_from(len).expect("instructions are short"), inst });
        }
    }

    /// Overwrites the bytes at `at`, emitted earlier.
    ///
    /// # Panics
    /// Panics if they are not all in the code.
    pub fn overwrite(&mut self, at: u64, bytes: &[u8]) {
        let at = usize::try_from(at).expect("code fits memory");
        self.code[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// The finished code, each reference to a label written.
    ///
    /// # Errors
    /// Returns a label never bound, or one further than a reference to it reaches.
    pub fn finish(mut self) -> Result<Code<I::Inst<S>, S>, Unpatched> {
        for &(at, label, fixup) in &self.fixups {
            let target = self.labels[label.0 as usize].ok_or(Unpatched::Unbound(label))?;
            I::patch(fixup, &mut self.code, at, target)?;
        }
        Ok(Code { bytes: self.code, relocs: self.relocs, listing: self.listing.unwrap_or_default() })
    }
}
