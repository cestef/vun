//! Copies of memory: the moves unrolled where they are few, a loop over words where they are
//! many, so that no code calls a `memcpy` the runtime may itself be written with.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Type, Value as Val, types};

use super::Lower;

/// The most bytes a copy moves unrolled.
const UNROLLED: u64 = 128;

/// The widths a copy moves in, widest first.
const WIDTHS: [Type; 4] = [types::I64, types::I32, types::I16, types::I8];

impl Lower<'_, '_> {
    pub(super) fn copy(&mut self, to: Val, from: Val, bytes: u64) {
        let word = WIDTHS[0].bytes();
        let looped = if bytes > UNROLLED { bytes / u64::from(word) * u64::from(word) } else { 0 };
        if looped > 0 {
            self.copy_words(to, from, looped);
        }
        let mut offset = looped;
        for ty in WIDTHS {
            while bytes - offset >= u64::from(ty.bytes()) {
                self.move_at(ty, to, from, offset);
                offset += u64::from(ty.bytes());
            }
        }
    }

    /// Moves the `bytes` at `offset` of `from` to those of `to`, one `ty` at a time.
    fn move_at(&mut self, ty: Type, to: Val, from: Val, offset: u64) {
        let offset = i32::try_from(offset).expect("copies are within a signed offset");
        let flags = Self::plain();
        let v = self.b.ins().load(ty, flags, from, offset);
        self.b.ins().store(flags, v, to, offset);
    }

    /// A loop moving `bytes`, a multiple of a word, from the start.
    fn copy_words(&mut self, to: Val, from: Val, bytes: u64) {
        let (head, body, exit) = (self.b.create_block(), self.b.create_block(), self.b.create_block());
        let at = self.b.append_block_param(head, self.ptr);
        let start = self.b.ins().iconst(self.ptr, 0);
        self.b.ins().jump(head, &[start.into()]);
        self.b.switch_to_block(head);
        let end = self.b.ins().iconst(self.ptr, i64::try_from(bytes).expect("copies fit a signed word"));
        let more = self.b.ins().icmp(IntCC::UnsignedLessThan, at, end);
        self.b.ins().brif(more, body, &[], exit, &[]);
        self.b.switch_to_block(body);
        let (src, dst) = (self.b.ins().iadd(from, at), self.b.ins().iadd(to, at));
        let flags = Self::plain();
        let word = self.b.ins().load(WIDTHS[0], flags, src, 0);
        self.b.ins().store(flags, word, dst, 0);
        let step = i64::from(WIDTHS[0].bytes());
        let next = self.b.ins().iadd_imm_s(at, step);
        self.b.ins().jump(head, &[next.into()]);
        self.b.switch_to_block(exit);
    }
}
