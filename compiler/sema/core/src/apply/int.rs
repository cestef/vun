//! What the primitives of `lang.int` compute (spec 04): the ranges of results, by interval
//! arithmetic, and the values, exactly. An integer operator is a call of one of them, through
//! the impls of `lang.int.ops`.

use vun_desugar::Narrowing;
use vun_primitives::int::Int;
use vun_sema_int::{Bitwise, Integer, Interval, Shift};
use vun_sema_ty::Value;
use vun_sema_ty::trap::Reason;
use vun_sema_ty::trap::Unevaluated;

use crate::env::Env;

/// What a bit operation on operands of no `uN` type reports (spec 04, rule 8).
const NOT_UNSIGNED: &str =
    "bit operations exist on `uN` types only: bring the value into one with `.wrap(uN)`";

/// What the primitives of `lang.int` compute (spec 04).
pub(crate) trait ApplyInt {
    /// The value of this primitive applied to `args` at compile time.
    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated>;

    /// The range of this operation's result on integers of `a` and `b`: empty for a division by
    /// a divisor that is always 0, which never gives a value.
    fn range(self, a: &Interval, b: &Interval) -> Result<Option<Interval>, Unevaluated>;

    /// The value of this operation on `x` of the range `a` and `y`.
    fn value(self, env: &Env<'_, '_>, a: &Interval, x: &Integer, y: &Integer) -> Result<Value, Unevaluated>;

    /// The direction of a shift primitive.
    fn shift(self) -> Option<Shift>;

    /// `x << n` or `x >> n`: in the `uN` type of `a`, the bits shifted out are lost and an amount
    /// past `N - 1` traps; a compile-time value shifts exactly, up to [`vun_sema_int::MAX_BITS`].
    fn shifted(a: &Interval, x: &Integer, n: &Integer, shift: Shift) -> Result<Integer, Unevaluated>;

    /// Whether a call with the compile-time arguments `args` may trap, where its `$` arguments
    /// decide: a division by a divisor whose range holds 0 (spec 04, rule 6), a shift of a `uN`
    /// value by an amount that may pass `N - 1` (rule 8), or of a compile-time value by an amount
    /// that may shift it past [`vun_sema_int::MAX_BITS`]. `None` for another primitive, which its
    /// markers describe.
    #[must_use]
    fn may_trap(self, args: &[Option<Value>]) -> Option<bool>;

    fn not_unsigned() -> Unevaluated;
}

impl ApplyInt for Int {
    fn apply(self, env: &Env<'_, '_>, args: &[Value]) -> Result<Value, Unevaluated> {
        match (self, args) {
            (Self::range, [ty]) => {
                let range = ty.as_type().and_then(|t| t.interval()).ok_or(Unevaluated::Unsupported)?;
                Ok(Value::Range(Some(range)))
            }
            (Self::magnitude, [Value::Range(range)]) => {
                Ok(Value::Range(range.as_ref().map(Interval::magnitude)))
            }
            (Self::negation, [Value::Range(Some(a))]) => Ok(Value::Range(Some(a.neg()))),
            (_, [Value::Range(Some(a)), Value::Range(Some(b))]) => self.range(a, b).map(Value::Range),
            (_, [Value::Range(Some(a)), Value::Range(Some(_)), Value::Int(x), Value::Int(y)]) => {
                self.value(env, a, x, y)
            }
            (Self::neg, [Value::Range(_), Value::Int(x)]) => Ok(Value::Int(&Integer::zero() - x)),
            (Self::abs, [Value::Range(_), Value::Int(x)]) => Ok(Value::Int(x.abs())),
            (Self::bit_not, [Value::Range(Some(a)), Value::Int(x)]) => {
                let bits = a.bitwise(a).and_then(|u| u.unsigned_width()).ok_or_else(Self::not_unsigned)?;
                Ok(Value::Int(x.complement(bits)))
            }
            // Narrowing an integer to the integer type `T` (rule 3).
            (Self::to | Self::wrap | Self::sat | Self::fits, [Value::Range(_), x, Value::Type(ty)]) => {
                let range = ty.interval().ok_or(Unevaluated::Unsupported)?;
                match self {
                    Self::to => x.narrow(&range, Narrowing::To),
                    Self::wrap => x.narrow(&range, Narrowing::Wrap),
                    Self::sat => x.narrow(&range, Narrowing::Sat),
                    _ => env.decide(x.narrow(&range, Narrowing::To).is_ok()),
                }
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }

    fn range(self, a: &Interval, b: &Interval) -> Result<Option<Interval>, Unevaluated> {
        let unknown = Unevaluated::Unsupported;
        let known = |range: Result<Interval, Unevaluated>| range.map(Some);
        match self {
            Self::sum => Ok(Some(a.add(b))),
            Self::difference => Ok(Some(a.sub(b))),
            Self::product => Ok(Some(a.mul(b))),
            Self::quotient => Ok(a.div(b).range),
            Self::remainder => Ok(a.rem(b).range),
            Self::bitwise => known(a.bitwise(b).ok_or_else(Self::not_unsigned)),
            Self::shifted_left | Self::shifted_right => {
                // A shift of a run-time value outside any `uN` type is reported; a compile-time
                // one by an amount not bounded here has no range known here.
                let shift = self.shift().ok_or(Unevaluated::Unsupported)?;
                known(
                    a.shifted(b, shift)
                        .ok_or_else(|| if a.value().is_some() { unknown } else { Self::not_unsigned() }),
                )
            }
            _ => Err(unknown),
        }
    }

    fn value(self, env: &Env<'_, '_>, a: &Interval, x: &Integer, y: &Integer) -> Result<Value, Unevaluated> {
        let n = match self {
            Self::add => x + y,
            Self::sub => x - y,
            Self::mul => x * y,
            Self::div => x.checked_div(y).ok_or_else(|| Unevaluated::trap(Reason::DivisionByZero))?,
            Self::rem => x.checked_rem(y).ok_or_else(|| Unevaluated::trap(Reason::DivisionByZero))?,
            Self::bit_and => x.bitwise(y, Bitwise::And),
            Self::bit_or => x.bitwise(y, Bitwise::Or),
            Self::bit_xor => x.bitwise(y, Bitwise::Xor),
            Self::shl | Self::shr => Self::shifted(a, x, y, self.shift().ok_or(Unevaluated::Unsupported)?)?,
            Self::equal => return env.decide(x == y),
            Self::unequal => return env.decide(x != y),
            Self::less => return env.decide(x < y),
            Self::less_equal => return env.decide(x <= y),
            Self::greater => return env.decide(x > y),
            Self::greater_equal => return env.decide(x >= y),
            Self::compare => return env.ordering(x.cmp(y)),
            _ => return Err(Unevaluated::Unsupported),
        };
        Ok(Value::Int(n))
    }

    fn shift(self) -> Option<Shift> {
        match self {
            Self::shl | Self::shifted_left => Some(Shift::Left),
            Self::shr | Self::shifted_right => Some(Shift::Right),
            _ => None,
        }
    }

    fn shifted(a: &Interval, x: &Integer, n: &Integer, shift: Shift) -> Result<Integer, Unevaluated> {
        let trap = || Unevaluated::trap(Reason::Shift);
        let Some(bits) = a.unsigned_width() else { return x.shifted(shift, n).ok_or_else(trap) };
        if n.is_negative() || *n >= Integer::from(i128::from(bits)) {
            return Err(trap());
        }
        let shifted = x.shifted(shift, n).ok_or_else(trap)?;
        shifted.modulo(&Integer::power_of_two(bits)).ok_or(Unevaluated::Unsupported)
    }

    fn may_trap(self, args: &[Option<Value>]) -> Option<bool> {
        let range = |i: usize| match args.get(i) {
            Some(Some(Value::Range(Some(r)))) => Some(r.clone()),
            _ => None,
        };
        match self {
            Self::div | Self::rem => {
                let zero = Interval::singleton(Integer::zero());
                Some(range(1).is_none_or(|divisor| divisor.contains(&zero)))
            }
            Self::shl | Self::shr => {
                let (Some(value), Some(amount), Some(shift)) = (range(0), range(1), self.shift()) else {
                    return Some(true);
                };
                let Some(bits) = value.unsigned_width() else {
                    return Some(value.shifted(&amount, shift).is_none());
                };
                let within = Interval::closed(Integer::zero(), Integer::from(i128::from(bits) - 1));
                Some(within.is_none_or(|w| !w.contains(&amount)))
            }
            _ => None,
        }
    }

    fn not_unsigned() -> Unevaluated {
        Unevaluated::trap(Reason::Reported { message: NOT_UNSIGNED.to_owned(), at: None })
    }
}
