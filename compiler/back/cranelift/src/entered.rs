//! A function compiled as an entry and a body (`Tails`): one code, the entry first, which the
//! symbol of the function names, then the body at an aligned offset, which tail calls jump to.

use vun_asm::{Code, Reloc};

use crate::compile::Function;

impl Function {
    /// The code of `self`, a body, after that of `entry`, which calls it: the offsets of the body's
    /// relocations and marks move to where it is. The frame of a call is the entry's and the
    /// body's, each with the frame pointer and return address `word` bytes each.
    pub(crate) fn entered(self, entry: Self, align: u32, word: u64) -> Self {
        let at =
            entry.code.bytes.len().next_multiple_of(usize::try_from(align).expect("alignments fit usize"));
        let shift = u64::try_from(at).expect("code fits");
        let mut bytes = entry.code.bytes;
        bytes.resize(at, 0);
        bytes.extend(self.code.bytes);
        let mut relocs = entry.code.relocs;
        relocs.extend(self.code.relocs.into_iter().map(|r| Reloc { offset: r.offset + shift, ..r }));
        let mut marks = entry.marks;
        marks.extend(self.marks.into_iter().map(|(offset, origin)| (offset + shift, origin)));
        let text = match (entry.text, self.text) {
            (Some(entry), Some(body)) => {
                Some(crate::compile::Listed { text: format!("{}\n{}", entry.text, body.text), ..body })
            }
            (entry, body) => entry.or(body),
        };
        Self {
            code: Code { bytes, relocs, listing: Vec::new() },
            body_at: Some(shift),
            frame: entry.frame + self.frame + 2 * word,
            marks,
            text,
        }
    }
}
