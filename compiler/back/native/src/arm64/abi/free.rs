//! The argument registers and the stack a call's values take, in order.

use vun_asm_arm64::{Reg, V};

use super::{ARGS, COMPOSITE_ALIGN, Carrier, Loc, WORD};

/// The argument registers still free and the stack taken, as the call standard counts them.
pub(super) struct Free {
    pub(super) ints: &'static [Reg],
    pub(super) fps: &'static [V],
    pub(super) stack: u64,
    /// Whether a scalar on the stack takes its own size rather than a word.
    pub(super) packed: bool,
}

impl Free {
    /// Takes `size` bytes of the stack at their alignment: at least a word, even where scalars
    /// are packed, and at most 16 bytes (AAPCS64, rule C.16).
    pub(super) fn stack(&mut self, size: u64, align: u64) -> u64 {
        let at = self.stack.next_multiple_of(align.clamp(WORD, COMPOSITE_ALIGN));
        self.stack = at + size.next_multiple_of(WORD);
        at
    }

    /// Where the halves of an integer of two words travel: the next even pair of general
    /// registers, else 16 bytes of the stack at their alignment, no general register being used
    /// after it (AAPCS64, rules C.9 to C.11).
    pub(super) fn pair(&mut self) -> [Loc; 2] {
        if (ARGS.len() - self.ints.len()) % 2 == 1 {
            self.ints = &self.ints[1..];
        }
        if let Some((&[low, high], rest)) = self.ints.split_first_chunk() {
            self.ints = rest;
            return [low, high].map(|r| Loc::Reg(Carrier::Int(r)));
        }
        self.ints = &[];
        let offset = self.stack(2 * WORD, 2 * WORD);
        [offset, offset + WORD].map(|offset| Loc::Stack { offset, bytes: WORD })
    }

    /// The next register of the class, else the stack: a word, or `bytes` at their own
    /// alignment where arguments are packed.
    pub(super) fn word(&mut self, float: bool, bytes: u64) -> Loc {
        let next = if float {
            self.fps.split_first().map(|(&v, rest)| {
                self.fps = rest;
                Carrier::Fp(v)
            })
        } else {
            self.ints.split_first().map(|(&r, rest)| {
                self.ints = rest;
                Carrier::Int(r)
            })
        };
        next.map_or_else(
            || {
                if self.packed {
                    let offset = self.stack.next_multiple_of(bytes);
                    self.stack = offset + bytes;
                    Loc::Stack { offset, bytes }
                } else {
                    Loc::Stack { offset: self.stack(WORD, WORD), bytes: WORD }
                }
            },
            Loc::Reg,
        )
    }
}
