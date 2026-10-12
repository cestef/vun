//! Object-like macros whose replacement is a constant become constants: an integer, a float or a
//! string, written as one literal or as an expression of literals, of other such macros and of
//! enum constants (`#define FLAGS (A | B << 2)`). Any other macro is not bound.

use std::collections::HashMap;

/// What a macro is worth.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Int(i128),
    Float(f64),
    /// The text of a string, which holds no zero byte.
    Text(String),
}

/// A binary operator on integers.
#[derive(Clone, Copy)]
enum Operator {
    Or,
    Xor,
    And,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl Operator {
    /// The operators of each level of precedence, from the loosest binding to the tightest.
    const LEVELS: [&[Self]; 6] = [
        &[Self::Or],
        &[Self::Xor],
        &[Self::And],
        &[Self::Shl, Self::Shr],
        &[Self::Add, Self::Sub],
        &[Self::Mul, Self::Div, Self::Rem],
    ];

    fn spelling(self) -> &'static str {
        match self {
            Self::Or => "|",
            Self::Xor => "^",
            Self::And => "&",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
        }
    }

    /// The result of the operator on `a` and `b`; none past the range of the values or by zero.
    fn apply(self, a: i128, b: i128) -> Option<i128> {
        match self {
            Self::Or => Some(a | b),
            Self::Xor => Some(a ^ b),
            Self::And => Some(a & b),
            Self::Shl => u32::try_from(b).ok().and_then(|b| a.checked_shl(b)),
            Self::Shr => u32::try_from(b).ok().and_then(|b| a.checked_shr(b)),
            Self::Add => a.checked_add(b),
            Self::Sub => a.checked_sub(b),
            Self::Mul => a.checked_mul(b),
            Self::Div => a.checked_div(b),
            Self::Rem => a.checked_rem(b),
        }
    }
}

/// How deep macros naming macros are followed, which stops a macro that names itself.
const DEPTH_MAX: usize = 16;

/// What names may stand for in a macro: the replacement lists of the object-like macros in scope,
/// and the constants of the enums.
pub(crate) struct Names<'a> {
    pub(crate) macros: &'a HashMap<String, Vec<String>>,
    pub(crate) constants: &'a HashMap<String, i128>,
}

impl Names<'_> {
    /// The value of the replacement `tokens`; none when they are anything else.
    pub(crate) fn value(&self, tokens: &[String]) -> Option<Value> {
        let mut parser = Parser { names: self, tokens, at: 0, depth: 0 };
        let value = parser.expression(0)?;
        (parser.at == tokens.len()).then_some(value)
    }
}

struct Parser<'a, 'n> {
    names: &'a Names<'n>,
    tokens: &'a [String],
    at: usize,
    depth: usize,
}

impl<'a> Parser<'a, '_> {
    fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    /// An expression whose operators bind at least as tightly as `BINARY[level]`.
    fn expression(&mut self, level: usize) -> Option<Value> {
        let Some(operators) = Operator::LEVELS.get(level) else { return self.unary() };
        let mut left = self.expression(level + 1)?;
        while let Some(operator) = operators.iter().find(|op| Some(op.spelling()) == self.peek()) {
            self.at += 1;
            let right = self.expression(level + 1)?;
            let (Value::Int(a), Value::Int(b)) = (&left, &right) else { return None };
            left = Value::Int(operator.apply(*a, *b)?);
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<Value> {
        match self.peek()? {
            "-" => {
                self.at += 1;
                match self.unary()? {
                    Value::Int(n) => n.checked_neg().map(Value::Int),
                    Value::Float(x) => Some(Value::Float(-x)),
                    Value::Text(_) => None,
                }
            }
            "+" => {
                self.at += 1;
                self.unary().filter(|v| !matches!(v, Value::Text(_)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Option<Value> {
        let token = self.peek()?;
        if token == "(" {
            self.at += 1;
            let inner = self.expression(0)?;
            (self.peek()? == ")").then(|| self.at += 1)?;
            return Some(inner);
        }
        if token.starts_with('"') {
            return self.text();
        }
        self.at += 1;
        if let Some(n) = Self::character(token).or_else(|| integer_literal(token)) {
            return Some(Value::Int(n));
        }
        if let Some(x) = float_literal(token) {
            return Some(Value::Float(x));
        }
        self.named(token)
    }

    /// A macro or an enum constant, by its name.
    fn named(&mut self, name: &str) -> Option<Value> {
        if let Some(&n) = self.names.constants.get(name) {
            return Some(Value::Int(n));
        }
        let replacement = self.names.macros.get(name)?;
        (self.depth < DEPTH_MAX).then_some(())?;
        let mut inner = Parser { names: self.names, tokens: replacement, at: 0, depth: self.depth + 1 };
        let value = inner.expression(0)?;
        (inner.at == replacement.len()).then_some(value)
    }

    /// One or more string literals in a row, which C makes one string.
    fn text(&mut self) -> Option<Value> {
        let mut text = String::new();
        while let Some(token) = self.peek().filter(|t| t.starts_with('"')) {
            text.push_str(&unquote(token)?);
            self.at += 1;
        }
        Some(Value::Text(text))
    }

    /// The value of a character literal of one plain character.
    fn character(token: &str) -> Option<i128> {
        let body = token.strip_prefix('\'')?.strip_suffix('\'')?;
        let mut chars = body.chars();
        let c = chars.next().filter(|c| *c != '\\')?;
        chars.next().is_none().then_some(i128::from(u32::from(c)))
    }
}

/// An integer literal in C's forms (decimal, `0x`, `0b`, octal, with `u` and `l` suffixes).
fn integer_literal(literal: &str) -> Option<i128> {
    let digits = literal.trim_end_matches(['u', 'U', 'l', 'L']);
    let (digits, radix) = if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        (hex, 16)
    } else if let Some(bin) = digits.strip_prefix("0b").or_else(|| digits.strip_prefix("0B")) {
        (bin, 2)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (&digits[1..], 8)
    } else {
        (digits, 10)
    };
    // C allows `'` between digits since C23.
    i128::from_str_radix(&digits.replace('\'', ""), radix).ok()
}

/// A decimal floating literal, with an `f` or `l` suffix or none.
fn float_literal(literal: &str) -> Option<f64> {
    let digits = literal.trim_end_matches(['f', 'F', 'l', 'L']);
    let starts = digits.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.');
    (starts && !digits.starts_with("0x") && (digits.contains('.') || digits.contains(['e', 'E'])))
        .then(|| digits.parse().ok())?
}

/// The text a string literal holds, for the escapes a header uses; none for a zero byte or an
/// escape not read.
fn unquote(literal: &str) -> Option<String> {
    let body = literal.strip_prefix('"')?.strip_suffix('"')?;
    let mut text = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        text.push(match chars.next()? {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            _ => return None,
        });
    }
    Some(text)
}
