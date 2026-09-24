//! Bounds at run time (`lo..hi`, `lo..=hi`, `lo..`): the low bound, the end past the last item
//! and whether there is one, as the layout of `Bounds(T)` places them; and the run of items of an
//! array or slice they select, `xs[a..b]`, checked to lie within it.

use vun_grammar::TokenKind;
use vun_lir::{Access, BinOp, Cond, Scalar, Value};
use vun_mir::Operand;
use vun_sema::layout::Parts;
use vun_sema::ty::{Ty, TyKind};

use super::FnLower;
use super::place::Reached;
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// The storage of a bound of `Bounds(item)`, and whether it is signed.
    fn bound(&self, item: Ty) -> (Scalar, bool) {
        self.l.types.int(item).unwrap_or((self.l.types.word, false))
    }

    /// The offsets of the low bound, the end and the flag telling there is one.
    fn fields(&self, bounds: Ty) -> Result<[u64; 3], Unlowered> {
        match &self.l.types.shape(bounds, self.at)?.parts {
            Parts::Sequence(offsets) if let [lo, end, has] = offsets.as_slice() => Ok([*lo, *end, *has]),
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
        let [at_lo, at_end, at_has] = self.fields(ty)?;
        let lo = self.int_operand(lo, scalar)?;
        let at = self.at_offset(dest, at_lo);
        self.b.store(lo, at, Access::Plain, self.at);
        let has = u128::from(hi.is_some());
        if let Some(hi) = hi {
            let mut end = self.int_operand(hi, scalar)?;
            // `lo..=hi` ends past `hi`.
            if op == TokenKind::DotDotEq {
                let one = self.b.constant(scalar, 1, self.at);
                end = self.b.binary(BinOp::Add, end, one, self.at);
            }
            let at = self.at_offset(dest, at_end);
            self.b.store(end, at, Access::Plain, self.at);
        }
        let has = self.b.constant(Scalar::I8, has, self.at);
        let at = self.at_offset(dest, at_has);
        self.b.store(has, at, Access::Plain, self.at);
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
        let (scalar, signed) =
            self.bound(ty.bounds().ok_or(Unlowered::unsupported(self.at, Form::Projection))?);
        let [at_lo, at_end, at_has] = self.fields(ty)?;
        let load = |this: &mut Self, offset: u64, s: Scalar| {
            let at = this.at_offset(bounds, offset);
            this.b.load(s, at, Access::Plain, this.at)
        };
        let lo = load(self, at_lo, scalar);
        let lo = self.resize(lo, scalar, signed, word);
        let end = load(self, at_end, scalar);
        let end = self.resize(end, scalar, signed, word);
        // Without an end, the run goes to the last item.
        let has = load(self, at_has, Scalar::I8);
        let end = self.b.push(
            vun_lir::InstKind::Select { cond: has, then: end, otherwise: len },
            Some(word),
            self.at,
        );
        let end = end.expect("a select has a value");
        let ordered = self.b.compare(Cond::ULe, lo, end, self.at);
        let inside = self.b.compare(Cond::ULe, end, len, self.at);
        let valid = self.b.binary(BinOp::And, ordered, inside, self.at);
        self.check(valid)?;
        let addr = self.scaled(from.addr, lo, stride);
        let count = self.b.binary(BinOp::Sub, end, lo, self.at);
        Ok(Reached { addr, ty: TyKind::slice(item), len: Some(count) })
    }
}
