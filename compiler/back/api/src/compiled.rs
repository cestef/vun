//! A function's machine code as the framework leaves it.

use vun_codec::{Decode, Encode};
use vun_dwarf::Step;
use vun_lir::Address;
use vun_origin::OriginId;

/// A function's machine code.
#[derive(Encode, Decode)]
pub struct Compiled<T> {
    pub code: vun_asm::Code<T, Address>,
    /// The bytes its frame takes below the frame pointer.
    pub frame: u64,
    /// Where the code of each instruction and terminator starts, with its origin: what line
    /// tables and listings read.
    pub marks: Vec<(u64, OriginId)>,
    /// The steps of its frame, where each takes effect: what unwinders read.
    pub unwind: Vec<(u64, Step)>,
}

impl<T> Compiled<T> {
    /// The bytes the frame takes as C compilers count them (`-fstack-usage`): below the frame
    /// pointer, with the saved frame pointer and the return address, `word` bytes each, above it.
    #[must_use]
    pub const fn size(&self, word: u64) -> u64 {
        self.frame + 2 * word
    }

    /// The same code without the listing of its instructions.
    #[must_use]
    pub fn unlisted<U>(self) -> Compiled<U> {
        let vun_asm::Code { bytes, relocs, .. } = self.code;
        let code = vun_asm::Code { bytes, relocs, listing: Vec::new() };
        Compiled { code, frame: self.frame, marks: self.marks, unwind: self.unwind }
    }

    /// A copy of the code without the listing of its instructions.
    #[must_use]
    pub fn to_unlisted<U>(&self) -> Compiled<U> {
        let code = vun_asm::Code {
            bytes: self.code.bytes.clone(),
            relocs: self.code.relocs.clone(),
            listing: Vec::new(),
        };
        Compiled { code, frame: self.frame, marks: self.marks.clone(), unwind: self.unwind.clone() }
    }

    /// The code of a function nothing reaches: none.
    #[must_use]
    pub fn unreached() -> Self {
        let code = vun_asm::Code { bytes: Vec::new(), relocs: Vec::new(), listing: Vec::new() };
        Self { code, frame: 0, marks: Vec::new(), unwind: Vec::new() }
    }
}
