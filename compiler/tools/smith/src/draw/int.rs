//! Integer expressions: a rule for each form, and the coercions that make operands fit what
//! an operator asks of them.

use super::Draw;
use crate::int::{BigInt, Range};
use crate::ir::{Arith, Compare, Expr, Kind, Narrow, Unary, Untyped};
use crate::ty::{Nominal, Ty};

/// The forms of integer expressions: each is drawn by its weight, which is 0 where it needs
/// something the expression has no depth left for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
enum Form {
    Literal,
    Local,
    Sum,
    Product,
    Divide,
    Bits,
    Shift,
    Negate,
    Narrow,
    Choose,
    Part,
    Call,
    Match,
    Unwrap,
    Truncate,
    Closure,
}

impl Form {
    const fn weight(self, depth: usize) -> usize {
        match self {
            Self::Literal => 6 + if depth == 0 { 20 } else { 0 },
            Self::Local => 6,
            _ if depth == 0 => 0,
            Self::Sum => 8,
            Self::Product => 4,
            Self::Divide => 4,
            Self::Bits => 5,
            Self::Shift => 3,
            Self::Negate => 2,
            Self::Narrow => 6,
            Self::Choose => 3,
            Self::Part => 4,
            Self::Call => 3,
            Self::Match => 3,
            Self::Unwrap => 3,
            Self::Truncate => 3,
            Self::Closure => 3,
        }
    }
}

impl Draw<'_> {
    /// An integer expression of any range.
    pub(super) fn int(&mut self, depth: usize) -> Expr {
        self.spent += 1;
        if self.spent > self.config.nodes {
            return self.int_literal();
        }
        let weights: Vec<usize> = Form::ALL.iter().map(|f| f.weight(depth)).collect();
        for _ in 0..4 {
            let form = Form::ALL[self.entropy.weighted(&weights)];
            if let Some(expr) = self.int_form(form, depth) {
                return expr;
            }
        }
        self.int_literal()
    }

    pub(super) fn int_literal(&mut self) -> Expr {
        let range = self.range();
        Expr::int(self.literal_in(&range))
    }

    fn int_form(&mut self, form: Form, depth: usize) -> Option<Expr> {
        let below = depth.saturating_sub(1);
        match form {
            Form::Literal => Some(self.int_literal()),
            Form::Local => self.local_of(&|ty| ty.range().is_some()),
            Form::Sum => {
                let op = *self.entropy.pick(&[Arith::Add, Arith::Sub]);
                let (left, right) = (self.int(below), self.int(below));
                self.arith(op, left, right)
            }
            Form::Product => {
                let (left, right) = (self.int(below), self.int(below));
                self.arith(Arith::Mul, left, right)
            }
            Form::Divide => {
                let op = *self.entropy.pick(&[Arith::Div, Arith::Rem]);
                let (left, right) = (self.int(below), self.int(below));
                let right = self.nonzero(right);
                self.arith(op, left, right)
            }
            Form::Bits => {
                let op = *self.entropy.pick(&[Arith::And, Arith::Or, Arith::Xor]);
                if self.entropy.chance(1, 6) {
                    let word = self.int(below);
                    let word = self.as_word(word);
                    return Expr::unary(Unary::BitNot, word).ok();
                }
                let (left, right) = (self.int(below), self.int(below));
                let (left, right) = (self.as_word(left), self.as_word(right));
                Expr::arith(op, left, right).ok()
            }
            Form::Shift => {
                let op = *self.entropy.pick(&[Arith::Shl, Arith::Shr]);
                let (value, amount) = (self.int(below), self.int(below));
                let value = self.as_word(value);
                let bits = value.ty.range().and_then(Range::word)?;
                let amount = self.coerced(amount, &Range::new(0, bits - 1));
                Expr::arith(op, value, amount).ok()
            }
            Form::Negate => Expr::unary(Unary::Neg, self.int(below)).ok(),
            Form::Narrow => {
                let (from, to) = (self.int(below), self.range());
                let how = *self.entropy.pick(&[Narrow::Wrap, Narrow::Sat, Narrow::To]);
                let how = if how == Narrow::To && !from.ty.range()?.within(&to) { Narrow::Wrap } else { how };
                Expr::narrow(how, from, to).ok()
            }
            Form::Choose => {
                let (condition, then, otherwise) = (self.bool(below), self.int(below), self.int(below));
                Expr::when(condition, then, otherwise).ok()
            }
            Form::Part => self.part(below, &|ty| ty.range().is_some()),
            Form::Call => self.call(below, &|ty| ty.range().is_some()),
            Form::Match => self.matched(None, below),
            Form::Truncate => self.truncated(below),
            Form::Closure => self.closed(None, below),
            Form::Unwrap => {
                let option = self.option_of_int(below)?;
                let Ty::Enum(_, variants) = &option.ty else { return None };
                let Ty::Int(held) = &variants[1][0] else { return None };
                let held = held.clone();
                let default = self.int(below);
                let default = self.coerced(default, &held);
                Expr::unwrap_or(option, default).ok()
            }
        }
    }

    /// `left op right`, or, where the result is too wide, of the operands brought down to types
    /// of their own.
    pub(super) fn arith(&mut self, op: Arith, left: Expr, right: Expr) -> Option<Expr> {
        match Expr::arith(op, left.clone(), right.clone()) {
            Err(Untyped::Width) => {
                let (left, right) = (self.narrowed(left), self.narrowed(right));
                Expr::arith(op, left, right).ok()
            }
            made => made.ok(),
        }
    }

    /// `operand` as a value of 64 bits or fewer.
    fn narrowed(&mut self, operand: Expr) -> Expr {
        let bits = *self.entropy.pick(&[8, 16, 32, 64]);
        let to = if self.entropy.chance(1, 2) { Range::unsigned(bits) } else { Range::signed(bits) };
        self.coerced(operand, &to)
    }

    /// `operand` as a value of `to`, by the narrowing a draw picks that cannot trap.
    pub(super) fn coerced(&mut self, operand: Expr, to: &Range) -> Expr {
        let Some(from) = operand.ty.range() else { return operand };
        if from.within(to) && self.entropy.chance(3, 4) {
            return operand;
        }
        let how = *self.entropy.pick(&[Narrow::Wrap, Narrow::Sat]);
        Expr::narrow(how, operand, to.clone()).expect("an integer narrows to an integer type")
    }

    /// `operand` as a `uN`: the type bit operations are on. Only a narrowing is trusted to have
    /// it: a condition narrows the range of a binding it compares (spec 04, rule 4), and the
    /// model does not follow that, so what reads a binding is wrapped as well.
    pub(super) fn as_word(&mut self, operand: Expr) -> Expr {
        if matches!(operand.kind, Kind::Narrow(..)) && operand.ty.range().and_then(Range::word).is_some() {
            return operand;
        }
        if let Some(own) = operand.ty.range().and_then(Range::word) {
            return Expr::narrow(Narrow::Wrap, operand, Range::unsigned(own))
                .expect("an integer narrows to its own type");
        }
        let bits = *self.entropy.pick(&[1, 3, 8, 16, 32, 64, 128]);
        self.coerced(operand, &Range::unsigned(bits))
    }

    /// `operand` as a divisor: a value the compiler knows is not zero.
    pub(super) fn nonzero(&mut self, operand: Expr) -> Expr {
        let Some(range) = operand.ty.range() else { return operand };
        if !range.contains(&BigInt::ZERO) {
            return operand;
        }
        let most = BigInt::from(1) << self.entropy.between(0, 70);
        let to = if self.entropy.chance(1, 4) { Range::new(-most.clone(), -1) } else { Range::new(1, most) };
        self.coerced(operand, &to)
    }

    /// An option of an integer: a local that holds one, or `try_to` of an integer.
    fn option_of_int(&mut self, depth: usize) -> Option<Expr> {
        let holds_int = |ty: &Ty| matches!(ty, Ty::Enum(Nominal::Option, v) if matches!(v[1][0], Ty::Int(_)));
        if self.entropy.chance(1, 2)
            && let Some(local) = self.local_of(&holds_int)
        {
            return Some(local);
        }
        let operand = self.int(depth);
        let to = self.range();
        Expr::try_to(operand, to).ok()
    }

    /// A boolean expression.
    pub(super) fn bool(&mut self, depth: usize) -> Expr {
        let below = depth.saturating_sub(1);
        if depth == 0 || self.entropy.chance(1, 8) {
            return self
                .local_of(&|ty| *ty == Ty::Bool)
                .unwrap_or_else(|| Expr::bool(self.entropy.chance(1, 2)));
        }
        if self.entropy.chance(1, 8)
            && let Some(option) = self.option_of_int(below)
        {
            return Expr::is_some(option).expect("an option is asked if it holds a value");
        }
        match self.entropy.below(6) {
            5 => self.float_compare(below),
            0 | 1 => {
                let op = *self.entropy.pick(Compare::ALL);
                let (left, right) = (self.int(below), self.int(below));
                if let Ok(compared) = Expr::compare(op, left.clone(), right.clone()) {
                    compared
                } else {
                    let (left, right) = (self.narrowed(left), self.narrowed(right));
                    Expr::compare(op, left, right).expect("integers of 64 bits compare")
                }
            }
            2 => {
                let op = *self.entropy.pick(&[crate::ir::Logic::And, crate::ir::Logic::Or]);
                Expr::logic(op, self.bool(below), self.bool(below)).expect("bools join")
            }
            3 => Expr::unary(Unary::Not, self.bool(below)).expect("bools negate"),
            _ => {
                let (condition, then, otherwise) = (self.bool(below), self.bool(below), self.bool(below));
                Expr::when(condition, then, otherwise).expect("bools branch")
            }
        }
    }
}
