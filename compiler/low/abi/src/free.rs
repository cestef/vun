//! The argument registers and the stack a call's values take, in order.

use crate::convention::{Pair, Rules, STACK_ALIGN, Short, Stack, WORD};
use crate::record::{Class, Piece};
use crate::{Carrier, Loc, Part};

/// The registers of one class, and how many are taken.
struct Bank<R: 'static> {
    regs: &'static [R],
    taken: usize,
}

/// What a call has left to place values in.
pub(crate) struct Free<'r, I: 'static, F: 'static> {
    rules: &'r Rules,
    ints: Bank<I>,
    floats: Bank<F>,
    stack: u64,
    /// Whether scalars lie on the stack at their own size, as a C call's do under some rules.
    packed: bool,
}

impl<R: Copy> Bank<R> {
    fn left(&self) -> usize {
        self.regs.len() - self.taken
    }

    fn next(&mut self) -> Option<R> {
        let reg = self.regs.get(self.taken).copied()?;
        self.taken += 1;
        Some(reg)
    }

    fn close(&mut self) {
        self.taken = self.regs.len();
    }
}

impl<'r, I: Copy, F: Copy> Free<'r, I, F> {
    pub(crate) fn new(rules: &'r Rules, ints: &'static [I], floats: &'static [F], packed: bool) -> Self {
        let packed = packed && rules.stack == Stack::Packed;
        Self {
            rules,
            ints: Bank { regs: ints, taken: 0 },
            floats: Bank { regs: floats, taken: 0 },
            stack: 0,
            packed,
        }
    }

    fn left(&self, class: Class) -> usize {
        match class {
            Class::Int => self.ints.left(),
            Class::Float => self.floats.left(),
        }
    }

    /// The next register of `class`, if one is left.
    pub(crate) fn next(&mut self, class: Class) -> Option<Carrier<I, F>> {
        match class {
            Class::Int => self.ints.next().map(Carrier::Int),
            Class::Float => self.floats.next().map(Carrier::Float),
        }
    }

    /// Registers for every piece, all or none; with none, the rules say whether the classes
    /// wanted stay open to later values.
    pub(crate) fn take(&mut self, pieces: &[Piece]) -> Option<Vec<Part<Carrier<I, F>>>> {
        let wanted = |class| pieces.iter().filter(|p| p.class == class).count();
        let short: Vec<Class> = [Class::Int, Class::Float]
            .into_iter()
            .filter(|&class| wanted(class) > self.left(class))
            .collect();
        if short.is_empty() {
            let parts = pieces.iter().map(|p| {
                let reg = self.next(p.class).expect("counted");
                Part { reg, offset: p.offset, bytes: p.bytes }
            });
            return Some(parts.collect());
        }
        for class in short {
            self.short(class);
        }
        None
    }

    /// What finding too few registers of `class` does to the rest, by the rules.
    fn short(&mut self, class: Class) {
        if self.rules.short == Short::Closes {
            match class {
                Class::Int => self.ints.close(),
                Class::Float => self.floats.close(),
            }
        }
    }

    /// Takes `size` bytes of the stack at their alignment, at least a word's and at most 16
    /// bytes', the size rounded up to words.
    pub(crate) fn stack(&mut self, size: u64, align: u64) -> u64 {
        let at = self.stack.next_multiple_of(align.clamp(WORD, STACK_ALIGN));
        self.stack = at + size.next_multiple_of(WORD);
        at
    }

    /// A scalar of `bytes` of `class`: in the next register of the class, else on the stack, a
    /// word or, packed, its own size at its own alignment.
    pub(crate) fn scalar(&mut self, class: Class, bytes: u64) -> Loc<Carrier<I, F>> {
        if let Some(reg) = self.next(class) {
            return Loc::Reg(reg);
        }
        if self.packed {
            let offset = self.stack.next_multiple_of(bytes);
            self.stack = offset + bytes;
            Loc::Stack { offset, bytes }
        } else {
            Loc::Stack { offset: self.stack(WORD, WORD), bytes: WORD }
        }
    }

    /// The halves of an integer of two words: two general registers as the rules pair them, else
    /// 16 bytes of the stack at their alignment.
    pub(crate) fn pair(&mut self) -> [Loc<Carrier<I, F>>; 2] {
        if self.rules.pair == Pair::Even && self.ints.taken % 2 == 1 {
            self.ints.next();
        }
        if self.ints.left() >= 2 {
            return [(); 2].map(|()| Loc::Reg(self.next(Class::Int).expect("counted")));
        }
        self.short(Class::Int);
        let offset = self.stack(2 * WORD, 2 * WORD);
        [offset, offset + WORD].map(|offset| Loc::Stack { offset, bytes: WORD })
    }

    /// The bytes of arguments on the stack, rounded to its alignment.
    pub(crate) fn stack_size(&self) -> u64 {
        self.stack.next_multiple_of(STACK_ALIGN)
    }

    /// How many floating-point registers carry arguments.
    pub(crate) fn floats_used(&self) -> u8 {
        u8::try_from(self.floats.taken).expect("fewer than 256 argument registers")
    }
}
