//! The program run by the model: what each root gives and so what `main` prints, as the spec
//! says it, with no compiler involved. A program that traps here was not made well typed, which
//! is a fault of the generator, not of what is held against it.

mod expr;
mod place;
mod stmt;

use stmt::Flow;

use std::fmt;

use crate::int::{BigInt, Range};
use crate::ir::{FnId, Fold, Function, LocalId, Piece, Program, Step, WORD};
use std::cell::RefCell;
use std::rc::Rc;

use vun_index::Idx;

use crate::value::{Cell, Value};

/// Why the model stopped running a program: what the typed forms rule out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    /// `.to` of a value the type does not hold.
    Narrowed(BigInt, Range),
    /// A division by zero.
    Zero,
    /// An index past the end.
    Index(BigInt, usize),
    /// A local read before it was bound.
    Unbound,
    /// A form met a value of another shape: the typing of the model is wrong.
    Shape,
    /// A `break` or `continue` outside a loop.
    Escape,
    /// More steps than a program is allowed.
    Fuel,
    /// A `?` found what returns from the function with the value: caught where it is called.
    Return(Value),
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Narrowed(n, range) => write!(f, "{n} is not in {}..={}", range.lo, range.hi),
            Self::Zero => f.write_str("a division by zero"),
            Self::Index(n, len) => write!(f, "the index {n} is past the end of {len} items"),
            Self::Unbound => f.write_str("a local read before it is bound"),
            Self::Shape => f.write_str("a value of another shape than the form's"),
            Self::Escape => f.write_str("a break or continue outside a loop"),
            Self::Fuel => f.write_str("more steps than the limit"),
            Self::Return(_) => f.write_str("a return outside a function"),
        }
    }
}

impl std::error::Error for Trap {}

/// A program being run.
pub struct Machine<'p> {
    program: &'p Program,
    fuel: u64,
    /// The values the iterator functions being run have yielded, the innermost last.
    yielded: Vec<Vec<Value>>,
}

/// What a function holds as it runs: each local a variable of its own, which a borrow lends.
struct Frame {
    locals: Vec<Option<Cell>>,
}

impl Frame {
    fn new(count: usize) -> Self {
        Self { locals: vec![None; count] }
    }

    /// The value a local holds.
    fn get(&self, id: LocalId) -> Result<Value, Trap> {
        let cell = self.locals[id.index()].as_ref().ok_or(Trap::Unbound)?;
        Ok(cell.0.borrow().clone())
    }

    /// A new variable for `id`, holding `value`.
    fn bind(&mut self, id: LocalId, value: Value) {
        self.locals[id.index()] = Some(Cell(Rc::new(RefCell::new(value))));
    }

    fn cell(&self, id: LocalId) -> Result<Cell, Trap> {
        self.locals[id.index()].clone().ok_or(Trap::Unbound)
    }
}

impl<'p> Machine<'p> {
    /// A machine for `program`, which stops after `fuel` steps.
    #[must_use]
    pub fn new(program: &'p Program, fuel: u64) -> Self {
        Self { program, fuel, yielded: Vec::new() }
    }

    /// The hash after each root, which is what `main` prints a line of.
    ///
    /// # Errors
    /// Returns why the program does not run to its end.
    pub fn hashes(&mut self) -> Result<Vec<u64>, Trap> {
        let mut hash = crate::lower::HASH_OFFSET;
        let mut hashes = Vec::new();
        for &root in &self.program.roots {
            let value = self.call(root, Vec::new())?;
            Self::fold(&mut hash, &value, &self.program.functions[root].result.fold())?;
            hashes.push(hash);
        }
        Ok(hashes)
    }

    /// `value` mixed into `hash`, as `fold` says.
    fn fold(hash: &mut u64, value: &Value, fold: &Fold) -> Result<(), Trap> {
        let mut mix = |part: u64| *hash = hash.wrapping_mul(crate::lower::HASH_PRIME).wrapping_add(part);
        match (fold, value) {
            (Fold::Int(range), Value::Int(n)) => {
                for &piece in Fold::pieces(range) {
                    mix(Self::piece(n, piece)?);
                }
            }
            (Fold::Bool, Value::Bool(b)) => mix(u64::from(*b)),
            (Fold::Float(format), Value::Float(x)) => mix(format.encoding(x.value())),
            (Fold::Parts(parts), _) => {
                let held = value.parts().ok_or(Trap::Shape)?;
                for (step, part) in parts {
                    let (Step::Field(at) | Step::Member(at) | Step::Item(at)) = *step;
                    Self::fold(hash, held.get(at).ok_or(Trap::Shape)?, part)?;
                }
            }
            (Fold::Variants(_, variants), Value::Variant(held, payload)) => {
                mix(u64::try_from(*held).map_err(|_| Trap::Shape)?);
                let parts = variants.get(*held).ok_or(Trap::Shape)?;
                for (part, value) in parts.iter().zip(payload) {
                    Self::fold(hash, value, part)?;
                }
            }
            _ => return Err(Trap::Shape),
        }
        Ok(())
    }

    /// What of an integer a piece takes, as 64 bits.
    fn piece(n: &BigInt, piece: Piece) -> Result<u64, Trap> {
        let word = Range::unsigned(WORD);
        match piece {
            Piece::Low => Self::bits(&word.wrap(n)),
            Piece::High => Self::bits(&word.wrap(&(n / Piece::word_size()))),
        }
    }

    fn bits(n: &BigInt) -> Result<u64, Trap> {
        u64::try_from(n).map_err(|_| Trap::Shape)
    }

    /// What `function` gives for `args`.
    pub(crate) fn call(&mut self, function: FnId, args: Vec<Value>) -> Result<Value, Trap> {
        let Function { locals, body, tail, .. } = &self.program.functions[function];
        let mut frame = Frame::new(locals.len());
        for (at, arg) in args.into_iter().enumerate() {
            frame.bind(LocalId::new(at), arg);
        }
        let ran = self.scope(body, Some(tail), &mut frame, function).and_then(|(flow, value)| match flow {
            Flow::Next => value.ok_or(Trap::Shape),
            Flow::Break(..) | Flow::Continue(_) => Err(Trap::Escape),
        });
        match ran {
            Err(Trap::Return(value)) => Ok(value),
            other => other,
        }
    }

    /// One step, taken from the fuel.
    pub(crate) fn step(&mut self) -> Result<(), Trap> {
        self.fuel = self.fuel.checked_sub(1).ok_or(Trap::Fuel)?;
        Ok(())
    }
}
