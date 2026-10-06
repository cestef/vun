//! The code of one function for a processor: bytes appended in order, labels, the references to
//! labels patched when the function ends, relocations, and what the first instruction emitted of
//! which the processor has no form needed.

use vun_codec::{Decode, Encode};
use vun_inline::Needed;
use vun_isa::{Need, Processor, Width};
use vun_object::RelocKind;

use crate::listing::Line;
use crate::{Isa, Label, Unpatched};

/// An address the linker writes into the code: `target` plus `addend`, at `offset`, as `kind`
/// says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Reloc<S> {
    pub offset: u64,
    pub target: S,
    pub kind: RelocKind,
    pub addend: i64,
}

/// A function's code as it is emitted.
pub struct Buffer<I: Isa, S: Copy> {
    code: Vec<u8>,
    relocs: Vec<Reloc<S>>,
    labels: Vec<Option<u64>>,
    fixups: Vec<(u64, Label, I::Fixup)>,
    /// What was emitted, when a listing is asked for.
    listing: Option<Vec<Line<I::Inst<S>>>>,
    /// The processor the code runs on, whose forms the assembler takes.
    processor: Processor<I::Feature>,
    /// What the first instruction of which the processor has no form needed.
    lacked: Option<Needed>,
}

/// A function's finished code.
#[derive(Encode, Decode)]
pub struct Code<T, S> {
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc<S>>,
    /// Empty unless a listing was asked for.
    pub listing: Vec<Line<T>>,
}

impl<I: Isa, S: Copy> Buffer<I, S> {
    /// A buffer of code for `processor`.
    #[must_use]
    pub fn new(processor: Processor<I::Feature>) -> Self {
        Self {
            code: Vec::new(),
            relocs: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
            listing: None,
            processor,
            lacked: None,
        }
    }

    /// A buffer of code for `processor` that lists what is emitted into it.
    #[must_use]
    pub fn listed(processor: Processor<I::Feature>) -> Self {
        Self { listing: Some(Vec::new()), ..Self::new(processor) }
    }

    #[must_use]
    pub fn processor(&self) -> Processor<I::Feature> {
        self.processor
    }

    /// Records that an instruction emitted needs `need`, which the processor lacks in every form
    /// taking its operands: the instruction is left out, and the code is refused.
    pub fn lack<Z: Width>(&mut self, need: Need<I::Feature, Z>) {
        self.lacked.get_or_insert(need.needed());
    }

    /// What the first instruction emitted of which the processor has no form needed, taken.
    pub fn lacked(&mut self) -> Option<Needed> {
        self.lacked.take()
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
        Label::new(u32::try_from(self.labels.len() - 1).expect("labels fit 32 bits"))
    }

    /// Binds `label` to where the next instruction goes.
    pub fn bind(&mut self, label: Label) {
        self.labels[label.number() as usize] = Some(self.here());
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

    /// Places at `label` a word of `bytes` bytes, aligned to them, that the linker fills with
    /// `target` plus `addend` as `kind` says: data the code loads, which control goes around.
    ///
    /// # Panics
    /// For a word wider than 8 bytes, which no relocation fills.
    pub fn word(&mut self, label: Label, bytes: u32, target: S, kind: RelocKind, addend: i64) {
        let padding = self.here().next_multiple_of(u64::from(bytes)) - self.here();
        self.put(&vec![0; usize::try_from(padding).expect("a word's alignment")]);
        self.bind(label);
        let at = self.here();
        self.relocate(at, target, kind, addend);
        self.put(&vec![0; bytes as usize]);
        if let Some(listing) = &mut self.listing {
            listing.push(Line::Word { at, len: bytes });
        }
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

    /// Replaces the instructions at `at`, emitted earlier, by those `emit` writes into a buffer of
    /// its own for the same processor, as long as them, binding and linking nothing: their bytes,
    /// their lines where the code is listed, and what the processor lacks of them.
    ///
    /// # Panics
    /// Panics if they bind a label or link anything, or their bytes are not all in the code.
    pub fn rewrite(&mut self, at: u64, emit: impl FnOnce(&mut Self)) {
        let mut written =
            Self { listing: self.listing.as_ref().map(|_| Vec::new()), ..Self::new(self.processor) };
        emit(&mut written);
        if let Some(needed) = written.lacked.take() {
            self.lacked.get_or_insert(needed);
        }
        self.replace(at, written.finish().expect("a rewrite binds no label"));
    }

    /// Replaces the instructions at `at` by `code`, as long as them and linking nothing.
    fn replace(&mut self, at: u64, code: Code<I::Inst<S>, S>) {
        assert!(code.relocs.is_empty(), "a replacement links nothing");
        self.overwrite(at, &code.bytes);
        for line in code.listing {
            if let Line::Inst { at: offset, inst, .. } = line {
                self.relist(at + offset, inst);
            }
        }
    }

    /// Lists the instruction at `at` as `inst`, where the code is listed: what its bytes, patched
    /// since it was emitted, now encode.
    pub fn relist(&mut self, at: u64, inst: I::Inst<S>) {
        let Some(listing) = &mut self.listing else { return };
        let listed = listing.iter_mut().find_map(|l| match l {
            Line::Inst { at: here, inst, .. } if *here == at => Some(inst),
            _ => None,
        });
        if let Some(listed) = listed {
            *listed = inst;
        }
    }

    /// The finished code, each reference to a label written.
    ///
    /// # Errors
    /// Returns a label never bound, or one further than a reference to it reaches.
    pub fn finish(mut self) -> Result<Code<I::Inst<S>, S>, Unpatched> {
        for &(at, label, fixup) in &self.fixups {
            let target = self.labels[label.number() as usize].ok_or(Unpatched::Unbound(label))?;
            I::patch(fixup, &mut self.code, at, target)?;
        }
        Ok(Code { bytes: self.code, relocs: self.relocs, listing: self.listing.unwrap_or_default() })
    }
}
