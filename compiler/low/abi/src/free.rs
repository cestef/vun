//! The argument registers and the stack a call's values take, in order.

use vun_lir::{Record, Scalar};

use crate::bank::{Bank, Span};
use crate::record::{Class, Piece};
use crate::{Carrier, Floats, Loc, Part, Rest, Rules, Short, Split, Stack};

/// What a call has left to place values in.
pub(crate) struct Free<'r, I: 'static, F: 'static> {
    rules: &'r Rules,
    ints: Bank<I>,
    floats: Bank<F>,
    stack: u64,
    /// Whether scalars lie on the stack at their own size, as a C call's do under some rules.
    packed: bool,
}

impl<'r, I: Copy, F: Copy> Free<'r, I, F> {
    pub(crate) fn new(rules: &'r Rules, ints: &'static [I], floats: &'static [F], packed: bool) -> Self {
        let span = match rules.floats {
            Floats::Next | Floats::Core => Span::One,
            Floats::Backfill => Span::Overlapping(Scalar::F32.bytes(0)),
        };
        Self {
            rules,
            ints: Bank::new(ints, Span::Words(rules.word)),
            floats: Bank::new(floats, span),
            stack: 0,
            packed: packed && rules.stack == Stack::Packed,
        }
    }

    /// Registers for every piece of a value aligned to `align` in memory, all or none: a register
    /// per piece, else the classes short of them.
    pub(crate) fn take(&mut self, pieces: &[Piece], align: u64) -> Result<Vec<Carrier<I, F>>, Vec<Class>> {
        let pair = self.rules.pair;
        let ints = Run::of(&self.ints, Class::Int, pieces, align, pair);
        let floats = Run::of(&self.floats, Class::Float, pieces, align, pair);
        let short = [(Class::Int, ints.is_none()), (Class::Float, floats.is_none())];
        let (Some(mut ints), Some(mut floats)) = (ints, floats) else {
            return Err(short.into_iter().filter(|&(_, short)| short).map(|(class, _)| class).collect());
        };
        let regs = pieces.iter().map(|p| match p.class {
            Class::Int => Carrier::Int(ints.next(&self.ints, p)),
            Class::Float => Carrier::Float(floats.next(&self.floats, p)),
        });
        let regs = regs.collect();
        self.ints.claim(ints.start, ints.units);
        self.floats.claim(floats.start, floats.units);
        Ok(regs)
    }

    /// What finding too few registers of each of `classes` does to the rest, by the rules.
    pub(crate) fn short(&mut self, classes: &[Class]) {
        if self.rules.short == Short::Closes {
            for class in classes {
                match class {
                    Class::Int => self.ints.close(),
                    Class::Float => self.floats.close(),
                }
            }
        }
    }

    /// The register a scalar of `bytes` of `class` takes, if one is left.
    pub(crate) fn one(&mut self, class: Class, bytes: u64) -> Option<Carrier<I, F>> {
        self.all(&[Piece { class, offset: 0, bytes }], bytes).map(|regs| regs[0])
    }

    /// Registers for every piece, or none and the rules applied to the classes short of them.
    fn all(&mut self, pieces: &[Piece], align: u64) -> Option<Vec<Carrier<I, F>>> {
        self.take(pieces, align).map_err(|short| self.short(&short)).ok()
    }

    /// A scalar of `bytes` of `class`: in its register, else on the stack in the words it takes
    /// or, packed, at its own size and alignment.
    pub(crate) fn scalar(&mut self, class: Class, bytes: u64) -> Loc<Carrier<I, F>> {
        if let Some(reg) = self.one(class, bytes) {
            return Loc::Reg(reg);
        }
        if self.packed {
            let offset = self.stack.next_multiple_of(bytes);
            self.stack = offset + bytes;
            return Loc::Stack { offset, bytes };
        }
        let slot = bytes.next_multiple_of(self.rules.word);
        Loc::Stack { offset: self.stack(slot, bytes), bytes: slot }
    }

    /// Leaves every later value on the stack, in the words it takes: variadic arguments under
    /// [`crate::Variadic::Stack`].
    pub(crate) fn spill(&mut self) {
        self.ints.close();
        self.floats.close();
        self.packed = false;
    }

    /// The halves of an integer of two halves of `half` bytes: two general registers, else the
    /// stack at the whole's alignment.
    pub(crate) fn wide(&mut self, half: u64) -> [Loc<Carrier<I, F>>; 2] {
        let halves = Piece::halves(half);
        if let Some(regs) = self.all(&halves, 2 * half) {
            return [regs[0], regs[1]].map(Loc::Reg);
        }
        let offset = self.stack(2 * half, 2 * half);
        [offset, offset + half].map(|offset| Loc::Stack { offset, bytes: half })
    }

    /// A record passed by value in `pieces`: in registers; else, as the rules let it, its first
    /// words in the general registers left and the rest on the stack; else copied onto the stack.
    pub(crate) fn record(&mut self, record: &Record, pieces: &[Piece]) -> Loc<Carrier<I, F>> {
        match self.take(pieces, record.align) {
            Ok(regs) => Loc::Split { parts: Part::of(regs, pieces), rest: None },
            Err(short) => self.straddle(record, pieces).unwrap_or_else(|| {
                self.short(&short);
                Loc::Copied { offset: self.stack(record.size, record.align), size: record.size }
            }),
        }
    }

    /// A record of words, its first in the general registers left and the rest on the stack,
    /// when the rules split it and nothing is on the stack yet; no general register is left
    /// after it.
    fn straddle(&mut self, record: &Record, pieces: &[Piece]) -> Option<Loc<Carrier<I, F>>> {
        let words = pieces.iter().all(|p| p.class == Class::Int);
        if self.rules.split != Split::WhileStackEmpty || self.stack > 0 || !words {
            return None;
        }
        let start = self.ints.next(record.align, self.rules.pair);
        let (head, tail) = pieces.split_at(self.ints.left(start).min(pieces.len()));
        let first = tail.first().filter(|_| !head.is_empty())?;
        let regs = (start..).map(|k| Carrier::Int(self.ints.reg(k))).take(head.len()).collect();
        self.ints.claim(start, head.len());
        self.ints.close();
        let bytes = record.size - first.offset;
        let rest = Rest { from: first.offset, offset: self.stack(bytes, record.align), bytes };
        Some(Loc::Split { parts: Part::of(regs, head), rest: Some(rest) })
    }

    /// Takes `size` bytes of the stack at their alignment, at least a word's and at most the
    /// stack's, the size rounded up to words.
    pub(crate) fn stack(&mut self, size: u64, align: u64) -> u64 {
        let at = self.stack.next_multiple_of(align.clamp(self.rules.word, self.rules.stack_align));
        self.stack = at + size.next_multiple_of(self.rules.word);
        at
    }

    /// The bytes of arguments on the stack, rounded to its alignment.
    pub(crate) fn stack_size(&self) -> u64 {
        self.stack.next_multiple_of(self.rules.stack_align)
    }

    /// How many floating-point registers carry arguments.
    pub(crate) fn floats_used(&self) -> u8 {
        u8::try_from(self.floats.used()).expect("fewer than 256 argument registers")
    }
}

/// The registers of one class the pieces of a value take: where they start, how many, and the
/// next piece's.
struct Run {
    start: usize,
    units: usize,
    at: usize,
}

impl Run {
    /// The run of `class` for `pieces` in `bank`, empty without any piece of it; `None` when too
    /// few registers are left.
    fn of<R: Copy>(
        bank: &Bank<R>,
        class: Class,
        pieces: &[Piece],
        align: u64,
        pair: crate::Pair,
    ) -> Option<Self> {
        let span = bank.span();
        let mut ours = pieces.iter().filter(|p| p.class == class).map(|p| span.units(p.bytes)).peekable();
        let Some(&first) = ours.peek() else { return Some(Self { start: 0, units: 0, at: 0 }) };
        let units = ours.sum();
        let start = bank.find(units, first, align, pair)?;
        Some(Self { start, units, at: start })
    }

    /// The register piece `p` takes, the pieces of the class taken in order.
    fn next<R: Copy>(&mut self, bank: &Bank<R>, p: &Piece) -> R {
        let reg = bank.reg(self.at);
        self.at += bank.span().units(p.bytes);
        reg
    }
}

impl<R> Part<R> {
    /// Each piece in its register.
    pub(crate) fn of(regs: Vec<R>, pieces: &[Piece]) -> Vec<Self> {
        let parts = regs.into_iter().zip(pieces);
        parts.map(|(reg, p)| Part { reg, offset: p.offset, bytes: p.bytes }).collect()
    }
}
