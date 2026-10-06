//! Fitting a line's operands to a form, as sema checks a block: each architecture says the width
//! they give and whether each position takes its operand, by the predicates its assembler encodes
//! with, so a block that checks is one the assembler encodes.

use vun_codec::{Decode, Encode};
use vun_inline::{Bound, ClassId, CondId, Line, Mem, Operand, RegRef, SizeId};

/// An operand as a line writes it, before a form takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Given<V, P, S> {
    /// A register, with the width its name gives, if it gives one.
    Reg {
        reg: RegRef,
        size: Option<SizeId>,
    },
    /// A value bound to a register of the class and width its type needs, or to the one named.
    Bound {
        bound: Bound<V, P>,
        class: ClassId,
        size: Option<SizeId>,
    },
    /// An integer; `None` when no 64 bits hold it.
    Imm(Option<i64>),
    Cond(CondId),
    /// One of the choices a position names: the kind of choice, in the order of the table's
    /// `Choice`, and its index among that kind's (`ish` of a barrier).
    Choice {
        of: ChoiceId,
        index: u8,
    },
    Mem {
        mem: Mem<V, P, S>,
        writeback: Writeback,
    },
    Label(Line),
    /// The address of something the linker places.
    Symbol(S),
    /// The registers a list position takes, each with the width its name gives: the rest of the
    /// line (`push r4, lr`).
    List(Vec<(RegRef, Option<SizeId>)>),
}

impl<V, P, S> Given<V, P, S> {
    /// The register class it names or binds, if it is a register.
    #[must_use]
    pub fn class(&self) -> Option<ClassId> {
        match self {
            Self::Reg { reg, .. } => Some(reg.class),
            Self::Bound { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// The width of register it names or binds, if it is a register of a known width.
    #[must_use]
    pub fn size(&self) -> Option<SizeId> {
        match self {
            Self::Reg { size, .. } | Self::Bound { size, .. } => *size,
            _ => None,
        }
    }

    /// The operand of a checked instruction it is.
    #[must_use]
    pub fn operand(self) -> Operand<V, P, S> {
        match self {
            Self::Reg { reg, .. } => Operand::Reg(reg),
            Self::Bound { bound, .. } => Operand::Bound(bound),
            Self::Imm(imm) => Operand::Imm(imm.unwrap_or_default()),
            Self::Cond(cond) => Operand::Cond(cond),
            Self::Choice { index, .. } => Operand::Choice(index),
            Self::Mem { mem, .. } => Operand::Mem(mem),
            Self::Label(line) => Operand::Label(line),
            Self::Symbol(symbol) => Operand::Symbol(symbol),
            Self::List(regs) => Operand::List(regs.into_iter().map(|(reg, _)| reg).collect()),
        }
    }
}

/// A kind of named choice, by its place in the table's `Choice`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ChoiceId(pub u8);

/// Whether a memory access writes the address it reached back to its base: not at all, before
/// the access (`[x1, 8]!`), or after it (`[x1], 8`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Writeback {
    #[default]
    No,
    Pre,
    Post,
}

/// What a mnemonic's spelling supplies: a condition as its suffix (`jz`, `b.eq`), or the width
/// of its access (`ldrb`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Fixed {
    pub cond: Option<CondId>,
    pub size: Option<SizeId>,
}

/// A value on a stack machine's stack: of the class of locals that holds it, or of any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Class(ClassId),
    Any,
}

/// A kind of block of a stack machine's code, by how a branch reaches it and what follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nest {
    /// A branch to it goes to its end.
    Plain,
    /// A branch to it goes to its start, so carries nothing.
    Loop,
    /// A plain block that an `else` may follow.
    If,
}

/// What an instruction of a stack machine does to the nesting of its code, which is blocks, each
/// giving at most one value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    None,
    /// Opens a block of the kind, giving `yields` where it gives a value.
    Open {
        nest: Nest,
        yields: Option<Slot>,
    },
    /// Ends the first arm of an `if` and opens the second.
    Reopen,
    /// Ends the innermost block.
    Close,
    /// Goes to the block `depth` blocks out of the innermost, which `always` does, so that what
    /// follows is not reached; otherwise only when a value it pops says so.
    Branch {
        depth: u32,
        always: bool,
    },
    /// Goes to one of the blocks at these depths, the last the default.
    Table(Vec<u32>),
}

/// What an instruction of a stack machine does to its stack: the values it pops, the last on
/// top, and the ones it pushes, and to the blocks around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub pops: Vec<Slot>,
    pub pushes: Vec<Slot>,
    pub scope: Scope,
}

/// A form's width and its operands, once a line's fit it, and its effect on the stack where the
/// instruction set has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fit<V, P, S> {
    pub size: SizeId,
    pub operands: Vec<Operand<V, P, S>>,
    pub stack: Option<Stack>,
}

impl<V, P, S> Fit<V, P, S> {
    /// A fit of an instruction set with no stack.
    #[must_use]
    pub const fn new(size: SizeId, operands: Vec<Operand<V, P, S>>) -> Self {
        Self { size, operands, stack: None }
    }

    /// `self`, its form affecting the stack as `stack` says.
    #[must_use]
    pub fn on(self, stack: Stack) -> Self {
        Self { stack: Some(stack), ..self }
    }
}

/// Why a form does not take a line's operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Misfit {
    /// It takes another number of operands.
    Count,
    /// No operand gives the instruction's width, and the form has several.
    Unsized,
    /// It is not defined at the width its operands give.
    Width,
    /// The operand at `at`, among those written, does not fit its position.
    Operand { at: usize, why: Why },
}

/// Why an operand does not fit a position, the nearest miss first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// An immediate the position's encoding or the operation's bits do not hold.
    Wide,
    /// A register of another class than the position's.
    Class,
    /// An operand of another kind, or width.
    Kind,
}

/// A position of a form, with its operand and that operand's place among those written: none
/// for a condition the mnemonic supplies (`jz`).
#[derive(Clone, Debug)]
pub struct Position<K, V, P, S> {
    pub kind: K,
    pub at: Option<usize>,
    pub given: Given<V, P, S>,
}

impl<K, V, P, S> Position<K, V, P, S> {
    /// That its operand does not fit it, for `why`.
    #[must_use]
    pub fn misfit(&self, why: Why) -> Misfit {
        Misfit::Operand { at: self.at.unwrap_or_default(), why }
    }
}

/// What a position of a form takes of a line, beyond one operand.
pub trait Takes: Copy {
    /// Whether it is a condition, which a mnemonic may supply (`jz`).
    fn cond(self) -> bool;

    /// Whether it is a list, which takes every register left on the line (`push r4, lr`).
    fn list(self) -> bool;
}

/// Each position of a form with its operand: a condition the mnemonic supplies fills a position
/// that is one, and a list takes the registers left.
///
/// # Errors
/// `Misfit::Count` when the line writes another number of operands than the positions leave, and
/// `Misfit::Operand` for an operand of a list that is no register.
pub fn positions<K: Takes, V: Clone, P: Clone, S: Clone>(
    kinds: impl Iterator<Item = K>,
    fixed: Fixed,
    givens: &[Given<V, P, S>],
) -> Result<Vec<Position<K, V, P, S>>, Misfit> {
    let mut rest = givens.iter().enumerate().peekable();
    let mut positions = Vec::new();
    for kind in kinds {
        match fixed.cond {
            Some(c) if kind.cond() => positions.push(Position { kind, at: None, given: Given::Cond(c) }),
            _ if kind.list() => {
                let at = rest.peek().map(|&(at, _)| at).ok_or(Misfit::Count)?;
                let regs = rest.by_ref().map(|(at, given)| match *given {
                    Given::Reg { reg, size } => Ok((reg, size)),
                    _ => Err(Misfit::Operand { at, why: Why::Kind }),
                });
                let given = Given::List(regs.collect::<Result<_, _>>()?);
                positions.push(Position { kind, at: Some(at), given });
            }
            _ => {
                let (at, given) = rest.next().ok_or(Misfit::Count)?;
                positions.push(Position { kind, at: Some(at), given: given.clone() });
            }
        }
    }
    if rest.next().is_some() {
        return Err(Misfit::Count);
    }
    Ok(positions)
}

/// The width of an instruction over `positions`, whose form is `defined` at these widths in
/// order: the one its spelling fixes, else the one the operands at its `sized` positions agree
/// on, else its first where it has one, where no position is sized, or where a value is bound at
/// a sized position with no known width (its type's width is the target's, and the target is
/// another architecture's), as nothing then says the width is another.
///
/// # Errors
/// `Misfit::Operand` for operands giving two widths, `Misfit::Width` for a form defined at none,
/// and `Misfit::Unsized` where several widths remain and no operand chooses one.
pub fn width<K: Copy, V, P, S>(
    positions: &[Position<K, V, P, S>],
    fixed: Fixed,
    sized: impl Fn(K) -> bool,
    mut defined: impl Iterator<Item = SizeId> + Clone,
) -> Result<SizeId, Misfit> {
    if let Some(size) = fixed.size {
        return Ok(size);
    }
    let mut known = None;
    for p in positions.iter().filter(|p| sized(p.kind)) {
        let Some(width) = p.given.size() else { continue };
        if known.is_some_and(|k| k != width) {
            return Err(p.misfit(Why::Kind));
        }
        known = Some(width);
    }
    if let Some(width) = known {
        return Ok(width);
    }
    let unknowable =
        positions.iter().any(|p| sized(p.kind) && matches!(p.given, Given::Bound { size: None, .. }));
    let asked = positions.iter().any(|p| sized(p.kind));
    if defined.clone().count() == 1 || !asked || unknowable {
        defined.next().ok_or(Misfit::Width)
    } else {
        Err(Misfit::Unsized)
    }
}
