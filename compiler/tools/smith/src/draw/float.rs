//! Floats: literals from where formats go wrong, operations, and conversions to and from
//! integers and the other formats.

use super::Draw;
use crate::float::Format;
use crate::ir::{Arith, Compare, Convert, Expr, Unary};
use crate::ty::Ty;

/// The forms of float expressions, each drawn by its weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
enum Form {
    Literal,
    Local,
    Arith,
    Negate,
    FromInt,
    FromFloat,
    Choose,
    Part,
    Call,
}

impl Form {
    const fn weight(self, depth: usize) -> usize {
        match self {
            Self::Literal => 6 + if depth == 0 { 20 } else { 0 },
            Self::Local => 6,
            _ if depth == 0 => 0,
            Self::Arith => 10,
            Self::Negate => 2,
            Self::FromInt => 4,
            Self::FromFloat => 4,
            Self::Choose => 2,
            Self::Part => 3,
            Self::Call => 3,
        }
    }
}

/// Values of a float type that lie on its edges: zeros, the least and the greatest numbers, and
/// where one rounds to another.
const EDGES: [f64; 22] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    1.5,
    -2.5,
    3.0,
    0.1,
    1e10,
    65504.0,
    65519.0,
    65520.0,
    6.1e-5,
    5.96e-8,
    2.98e-8,
    3.402_823_5e38,
    1.4e-45,
    1.175_494_35e-38,
    1e-320,
    1.797_693_134_862_315_7e308,
    4_503_599_627_370_497.0,
];

impl Draw<'_> {
    /// A float literal of `format`: an edge, a whole number, or any finite value.
    fn float_literal(&mut self, format: Format) -> Expr {
        let value = match self.entropy.below(4) {
            0 | 1 => *self.entropy.pick(&EDGES),
            2 => {
                let whole = self.entropy.bits() >> self.entropy.between(0, 63);
                #[allow(clippy::cast_precision_loss, reason = "a float literal of any magnitude, rounded")]
                let magnitude = whole as f64;
                magnitude * if self.entropy.chance(1, 2) { -1.0 } else { 1.0 }
            }
            _ => {
                let any = f64::from_bits(self.entropy.bits());
                if any.is_finite() { any } else { 2.0 }
            }
        };
        Expr::float(format, value)
    }

    /// A float expression of `format`.
    pub(super) fn float(&mut self, format: Format, depth: usize) -> Expr {
        let weights: Vec<usize> = Form::ALL.iter().map(|f| f.weight(depth)).collect();
        for _ in 0..4 {
            let form = Form::ALL[self.entropy.weighted(&weights)];
            if let Some(expr) = self.float_form(format, form, depth) {
                return expr;
            }
        }
        self.float_literal(format)
    }

    fn float_form(&mut self, format: Format, form: Form, depth: usize) -> Option<Expr> {
        let below = depth.saturating_sub(1);
        let own = |ty: &Ty| *ty == Ty::Float(format);
        match form {
            Form::Literal => Some(self.float_literal(format)),
            Form::Local => self.local_of(&own),
            Form::Arith => {
                let op = *self.entropy.pick(&[Arith::Add, Arith::Sub, Arith::Mul, Arith::Div, Arith::Rem]);
                Expr::arith(op, self.float(format, below), self.float(format, below)).ok()
            }
            Form::Negate => Expr::unary(Unary::Neg, self.float(format, below)).ok(),
            Form::FromInt => {
                let operand = self.int(below);
                Self::nearest(operand, format)
            }
            Form::FromFloat => {
                let from = *self.entropy.pick(Format::ALL);
                let operand = self.float(from, below);
                Self::nearest(operand, format)
            }
            Form::Choose => {
                let (condition, then, otherwise) =
                    (self.bool(below), self.float(format, below), self.float(format, below));
                Expr::when(condition, then, otherwise).ok()
            }
            Form::Part => self.part(below, &own),
            Form::Call => self.call(below, &own),
        }
    }

    /// `operand` as the float `format`, which only `f32` converts to `f16` in (backends have no
    /// other conversion), so anything else goes through it.
    fn nearest(operand: Expr, format: Format) -> Option<Expr> {
        let single = Ty::Float(Format::F32);
        let through = format == Format::F16
            && match &operand.ty {
                Ty::Float(from) => *from != Format::F32,
                Ty::Int(_) => true,
                _ => false,
            };
        let operand = if through { Expr::convert(Convert::Nearest, operand, single).ok()? } else { operand };
        Expr::convert(Convert::Nearest, operand, Ty::Float(format)).ok()
    }

    /// A comparison of two floats of one format.
    pub(super) fn float_compare(&mut self, depth: usize) -> Expr {
        let format = *self.entropy.pick(Format::ALL);
        let op = *self.entropy.pick(Compare::ALL);
        Expr::compare(op, self.float(format, depth), self.float(format, depth))
            .expect("floats of one format compare")
    }

    /// An integer from a float, clamped.
    pub(super) fn truncated(&mut self, depth: usize) -> Option<Expr> {
        let format = *self.entropy.pick(Format::ALL);
        let operand = self.float(format, depth);
        let to = self.range();
        Expr::convert(Convert::Truncate, operand, Ty::Int(to)).ok()
    }
}
