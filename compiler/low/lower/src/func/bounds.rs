//! Bounds at run time (`lo..hi`, `lo..=hi`, `lo..`): the low bound, the second bound and how
//! they end ([`BoundsEnd`]), as the layout of `Bounds(T)` places them; the primitives of
//! `lang.bounds` over them; and the run of items of an array or slice they select, `xs[a..b]`,
//! checked to lie within it.

use vun_grammar::TokenKind;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_primitives::bounds::Bounds;
use vun_sema::layout::{BoundsEnd, Parts};
use vun_sema_ty::{Ty, TyKind};

use super::FnLower;
use super::place::Reached;
use crate::unlowered::{Form, Unlowered};

/// Bounds read from memory.
struct Loaded {
    lo: Value,
    /// The second bound: the end past the last item where `before`, the last where `through`.
    end: Value,
    open: Value,
    before: Value,
    through: Value,
    scalar: Scalar,
    signed: bool,
}

impl FnLower<'_, '_, '_> {
    /// The storage of a bound of `Bounds(item)`, and whether it is signed.
    fn bound(&self, item: Ty) -> (Scalar, bool) {
        self.l.types.int(item).unwrap_or((self.l.types.word, false))
    }

    /// The offsets of the low bound, the second bound and how they end.
    fn fields(&self, bounds: Ty) -> Result<[u64; 3], Unlowered> {
        match &self.l.types.shape(bounds, self.at)?.parts {
            Parts::Sequence(offsets) if let [lo, end, how] = offsets.as_slice() => Ok([*lo, *end, *how]),
            _ => Err(Unlowered::unsupported(self.at, Form::Projection)),
        }
    }

    /// Writes the bounds `lo op hi` into `dest`, of type `Bounds(T)`.
    pub(crate) fn range(
        &mut self,
        op: TokenKind,
        lo: &Operand,
        hi: Option<&Operand>,
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let Some(dest) = dest else { return Ok(()) };
        let item = ty.bounds().ok_or(Unlowered::unsupported(self.at, Form::Constant))?;
        let (scalar, _) = self.bound(item);
        let [at_lo, at_end, at_how] = self.fields(ty)?;
        let lo = self.int_operand(lo, scalar)?;
        let at = self.at_offset(dest, at_lo);
        self.b.store(lo, at, Access::Plain, self.at);
        let how = match hi {
            None => BoundsEnd::Open,
            Some(_) if op == TokenKind::DotDotEq => BoundsEnd::Through,
            Some(_) => BoundsEnd::Before,
        };
        if let Some(hi) = hi {
            let end = self.int_operand(hi, scalar)?;
            let at = self.at_offset(dest, at_end);
            self.b.store(end, at, Access::Plain, self.at);
        }
        let how = self.b.constant(Scalar::I8, how.tag(), self.at);
        let at = self.at_offset(dest, at_how);
        self.b.store(how, at, Access::Plain, self.at);
        Ok(())
    }

    /// The bounds of type `ty` at `addr`.
    fn load_bounds(&mut self, addr: Value, ty: Ty) -> Result<Loaded, Unlowered> {
        let (scalar, signed) =
            self.bound(ty.bounds().ok_or(Unlowered::unsupported(self.at, Form::Projection))?);
        let [at_lo, at_end, at_how] = self.fields(ty)?;
        let load = |this: &mut Self, offset: u64, s: Scalar| {
            let at = this.at_offset(addr, offset);
            this.b.load(s, at, Access::Plain, this.at)
        };
        let (lo, end, how) =
            (load(self, at_lo, scalar), load(self, at_end, scalar), load(self, at_how, Scalar::I8));
        let mut is = |end: BoundsEnd| {
            let tag = self.b.constant(Scalar::I8, end.tag(), self.at);
            self.b.compare(Cond::Eq, how, tag, self.at)
        };
        let (open, before, through) = (is(BoundsEnd::Open), is(BoundsEnd::Before), is(BoundsEnd::Through));
        Ok(Loaded { lo, end, open, before, through, scalar, signed })
    }

    /// `if_before` where the bounds end before their second bound, `if_through` where they end
    /// at it, false without an end.
    fn by_end(&mut self, bounds: &Loaded, if_before: Value, if_through: Value) -> Value {
        let before = self.b.binary(BinOp::And, bounds.before, if_before, self.at);
        let through = self.b.binary(BinOp::And, bounds.through, if_through, self.at);
        self.b.binary(BinOp::Or, before, through, self.at)
    }

    /// The primitives of `lang.bounds`, `runtime` their run-time arguments: the bounds, then for
    /// `ends` and `after` the item.
    pub(crate) fn bounded(
        &mut self,
        p: Bounds,
        runtime: &[&Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Primitive);
        let arg = |i: usize| runtime.get(i).copied().ok_or_else(unsupported);
        let bounds_ty = self.operand_ty(arg(0)?).ok_or_else(unsupported)?;
        let addr = self.operand_addr(arg(0)?)?;
        let bounds = self.load_bounds(addr, bounds_ty)?;
        let cmp = |c: Cond| if bounds.signed { c } else { c.unsigned() };
        if p == Bounds::first {
            let Some(dest) = dest else { return Ok(()) };
            self.b.store(bounds.lo, dest, Access::Plain, self.at);
            return Ok(());
        }
        if p == Bounds::empty {
            let (at_or_below, below) = (
                self.b.compare(cmp(Cond::SLe), bounds.end, bounds.lo, self.at),
                self.b.compare(cmp(Cond::SLt), bounds.end, bounds.lo, self.at),
            );
            let holds = self.by_end(&bounds, at_or_below, below);
            return self.condition(holds, dest, ty);
        }
        // An item of bounds ending before their second bound is below it, so the next one
        // stays within the items' storage.
        let item = self.int_operand(arg(1)?, bounds.scalar)?;
        let one = self.b.constant(bounds.scalar, 1, self.at);
        let next = self.b.binary(BinOp::Add, item, one, self.at);
        let (next_ends, item_ends) = (
            self.b.compare(Cond::Eq, next, bounds.end, self.at),
            self.b.compare(Cond::Eq, item, bounds.end, self.at),
        );
        let last = self.by_end(&bounds, next_ends, item_ends);
        if p == Bounds::ends {
            return self.condition(last, dest, ty);
        }
        let Some(dest) = dest else { return Ok(()) };
        let after = self.b.select(last, item, next, self.at);
        self.b.store(after, dest, Access::Plain, self.at);
        Ok(())
    }

    /// The run of items of `from`, whose items are `item`, `stride` bytes apart and `len` many,
    /// that the bounds at `bounds` select: its address and count, as a slice.
    pub(crate) fn run(
        &mut self,
        from: Reached,
        item: Ty,
        stride: u64,
        len: Value,
        bounds: Value,
        ty: Ty,
    ) -> Result<Reached, Unlowered> {
        let word = self.l.types.word;
        let b = self.load_bounds(bounds, ty)?;
        let lo = self.word_index(b.lo, b.scalar, b.signed)?;
        // Open bounds leave their end unwritten, which only a checked end reads.
        let end = if self.wider_than_word(b.scalar) {
            let zero = self.b.constant(b.scalar, 0, self.at);
            self.b.select(b.open, zero, b.end, self.at)
        } else {
            b.end
        };
        let second = self.word_index(end, b.scalar, b.signed)?;
        // The end past the last item; bounds through the greatest word select past every run,
        // which the test of the last item against the length catches.
        let one = self.b.constant(word, 1, self.at);
        let past = self.b.binary(BinOp::Add, second, one, self.at);
        let end = self.b.select(b.through, past, second, self.at);
        let (at_most, below) = (
            self.b.compare(Cond::ULe, second, len, self.at),
            self.b.compare(Cond::ULt, second, len, self.at),
        );
        let bounded = self.by_end(&b, at_most, below);
        // Without an end, the run goes to the last item.
        let inside = self.b.binary(BinOp::Or, bounded, b.open, self.at);
        let end = self.b.select(b.open, len, end, self.at);
        let ordered = self.b.compare(Cond::ULe, lo, end, self.at);
        let valid = self.b.binary(BinOp::And, ordered, inside, self.at);
        self.check(valid)?;
        let addr = self.scaled(from.addr, lo, stride);
        let count = self.b.binary(BinOp::Sub, end, lo, self.at);
        Ok(Reached { addr, ty: TyKind::slice(item), len: Some(count) })
    }
}
