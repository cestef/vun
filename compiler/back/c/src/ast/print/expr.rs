//! Expressions: atoms bare, compound expressions in parentheses where they are operands.

use std::fmt::{self, Write};

use super::Printer;
use crate::ast::{Binary, Expr, Suffix, Unary};

impl Printer<'_, '_, '_> {
    pub(super) fn expr(&mut self, e: &Expr) -> fmt::Result {
        match e {
            Expr::Name(ident) => self.ident(*ident),
            Expr::Literal { value, suffix } => write!(
                self.f,
                "{value}{}",
                match suffix {
                    Suffix::Unsigned => "u",
                    Suffix::Long => "ul",
                    Suffix::LongLong => "ull",
                }
            ),
            Expr::Small(n) => write!(self.f, "{n}"),
            Expr::Float { bits, value } => self.float(*bits, *value),
            Expr::Unary(op, e) => {
                self.f.write_str(match op {
                    Unary::Deref => "*",
                    Unary::Address => "&",
                })?;
                self.operand(e)
            }
            Expr::Binary(op, a, b) => {
                self.operand(a)?;
                write!(self.f, " {} ", op.text())?;
                self.operand(b)
            }
            Expr::Cast(ty, e) => {
                self.f.write_char('(')?;
                self.ty(ty)?;
                self.f.write_char(')')?;
                self.operand(e)
            }
            Expr::Call(callee, args) => {
                self.operand(callee)?;
                if args.is_empty() {
                    return self.f.write_str("()");
                }
                self.params(args.iter(), Self::expr)
            }
            Expr::Select(c, a, b) => {
                self.operand(c)?;
                self.f.write_str(" ? ")?;
                self.operand(a)?;
                self.f.write_str(" : ")?;
                self.operand(b)
            }
            Expr::Sizeof(ty) => {
                self.f.write_str("sizeof(")?;
                self.ty(ty)?;
                self.f.write_char(')')
            }
            Expr::Init(items) => {
                self.f.write_str("{ ")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.f.write_str(", ")?;
                    }
                    self.expr(item)?;
                }
                self.f.write_str(" }")
            }
        }
    }

    /// An expression another one reads: parenthesized unless it is an atom.
    pub(super) fn operand(&mut self, e: &Expr) -> fmt::Result {
        let atom = match e {
            Expr::Small(n) => *n >= 0,
            Expr::Name(_) | Expr::Literal { .. } | Expr::Float { .. } | Expr::Call(..) | Expr::Sizeof(_) => {
                true
            }
            Expr::Unary(..) | Expr::Binary(..) | Expr::Cast(..) | Expr::Select(..) | Expr::Init(_) => false,
        };
        if atom {
            return self.expr(e);
        }
        self.f.write_char('(')?;
        self.expr(e)?;
        self.f.write_char(')')
    }

    /// A finite float as an exact hexadecimal literal, `f`-suffixed for a `float`.
    fn float(&mut self, bits: u32, value: u64) -> fmt::Result {
        let x = if bits == 32 {
            #[expect(clippy::cast_possible_truncation, reason = "a float's bits are its width")]
            f64::from(f32::from_bits(value as u32))
        } else {
            f64::from_bits(value)
        };
        let raw = x.to_bits();
        let sign = if raw >> 63 == 1 { "-" } else { "" };
        let exponent = i64::try_from((raw >> 52) & 0x7ff).expect("11 bits");
        let mantissa = raw & ((1 << 52) - 1);
        let (lead, power) = match exponent {
            0 if mantissa == 0 => (0, 0),
            0 => (0, -1022),
            e => (1, e - 1023),
        };
        let digits = format!("{mantissa:013x}");
        let digits = digits.trim_end_matches('0');
        let dot = if digits.is_empty() { "" } else { "." };
        let suffix = if bits == 32 { "f" } else { "" };
        write!(self.f, "{sign}0x{lead}{dot}{digits}p{power:+}{suffix}")
    }
}

impl Binary {
    const fn text(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::And => "&",
            Self::Or => "|",
            Self::Xor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Either => "||",
        }
    }
}
