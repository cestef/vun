//! Function bodies: locals, and basic blocks of statements ending in a terminator (spec 13,
//! section 3). Every local, statement and terminator carries the origin that produced it.

use vun_codec::{Decode, Encode};
use vun_index::{IndexVec, index};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::item::DeclRef;
use vun_sema::member::Members;
use vun_sema::ty::{Ty, TyKind};

use crate::place::Place;
use crate::rvalue::{Aggregate, Operand, Rvalue};

index! {
    pub struct Local;
    pub struct BlockId;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Body {
    pub locals: IndexVec<Local, LocalDecl>,
    /// The first locals, in order.
    pub params: Vec<Local>,
    /// Where the result is written before returning.
    pub ret: Local,
    pub blocks: IndexVec<BlockId, Block>,
    pub origin: OriginId,
}

impl Body {
    /// Where ending the frame suspended at the yield resuming at `resume` goes on.
    #[must_use]
    pub fn cancel_of(&self, resume: BlockId) -> Option<BlockId> {
        self.blocks.iter().find_map(|(_, b)| match b.term.kind {
            TerminatorKind::Yield { resume: r, cancel, .. } if r == resume => Some(cancel),
            _ => None,
        })
    }

    /// The body of the function a variant's constructor is as a value, `E.fs`: its payload as
    /// parameters, the variant as the result. `None` when the payload's types are not known.
    #[must_use]
    pub fn constructor(cx: &Cx<'_>, ty: Ty, name: Symbol) -> Option<Self> {
        let TyKind::Nominal(nominal) = &*ty else { return None };
        let origin = nominal.literal.expr.origin;
        let members = cx.get::<Members>(&ty)?;
        let variant = members.variants.iter().find(|v| v.name == name)?;
        let local = |ty| LocalDecl { name: None, ty: Some(ty), role: Role::Held, origin };
        let mut locals = IndexVec::new();
        let ret = locals.push(local(ty));
        let params: Vec<Local> = variant
            .payload
            .iter()
            .map(|p| Some(locals.push(local(*p.as_ref().ok()?))))
            .collect::<Option<_>>()?;
        let parts = params.iter().map(|&p| Operand::Place(Place::of(p))).collect();
        let value = Rvalue::Aggregate { kind: Aggregate::Variant { ty, name }, parts };
        let stmts = vec![Stmt { place: Place::of(ret), value, origin }];
        let block = Block { stmts, term: Terminator { kind: TerminatorKind::Return, origin } };
        Some(Self { locals, params, ret, blocks: IndexVec::from(vec![block]), origin })
    }

    /// Whether this is a coroutine function's body: one that yields.
    #[must_use]
    pub fn suspends(&self) -> bool {
        self.blocks.iter().any(|(_, b)| matches!(b.term.kind, TerminatorKind::Yield { .. }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct LocalDecl {
    /// `None` for a temporary.
    pub name: Option<Symbol>,
    /// Its type, where known.
    pub ty: Option<Ty>,
    pub role: Role,
    pub origin: OriginId,
}

/// What a local holds for the checks, and for code generation where it lives.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Role {
    /// A value, which is consumed as a whole.
    Held,
    /// A value a pattern takes apart: its parts, bound or dropped, carry what it held.
    Unpacked,
    /// The storage of the top-level binding `decl`, which outlives every call: a borrow of it
    /// escapes nowhere.
    Static(DeclRef),
    /// The storage of the `mut` top-level binding `decl`: a static, as `Static`, holding its
    /// initial value, whose value later is known at run time only.
    StaticMut(DeclRef),
}

impl Role {
    /// Whether the local is a top-level binding's storage.
    #[must_use]
    pub fn is_static(&self) -> bool {
        self.global().is_some()
    }

    /// The top-level binding whose storage the local is.
    #[must_use]
    pub fn global(&self) -> Option<&DeclRef> {
        match self {
            Self::Static(decl) | Self::StaticMut(decl) => Some(decl),
            Self::Held | Self::Unpacked => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub term: Terminator,
}

/// `place = value`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Stmt {
    pub place: Place,
    pub value: Rvalue,
    pub origin: OriginId,
}

impl Stmt {
    /// The locals it reads: through operands, borrows, tests, the projections of what it writes
    /// and the indices places use.
    #[must_use]
    pub fn reads(&self) -> Vec<Local> {
        let mut out = Vec::new();
        self.place.read_by(false, &mut out);
        match &self.value {
            Rvalue::Ref { place, .. }
            | Rvalue::IsVariant { place, .. }
            | Rvalue::Resume { frame: place, .. }
            | Rvalue::End { frame: place } => place.read_by(true, &mut out),
            value => {
                for operand in value.operands() {
                    if let Operand::Place(place) = operand {
                        place.read_by(true, &mut out);
                    }
                }
            }
        }
        out
    }

    /// The local it writes whole, which holds nothing it held before.
    #[must_use]
    pub fn writes_whole(&self) -> Option<Local> {
        self.place.projection.is_empty().then_some(self.place.local)
    }

    /// The local it borrows, which a borrow may reach from anywhere after.
    #[must_use]
    pub fn borrows(&self) -> Option<Local> {
        match &self.value {
            Rvalue::Ref { place, .. } => Some(place.local),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub origin: OriginId,
}

impl TerminatorKind {
    /// The locals it reads: its operands, and what a deferred statement consumes.
    #[must_use]
    pub fn reads(&self) -> Vec<Local> {
        let mut out = Vec::new();
        for operand in self.operands() {
            if let Operand::Place(place) = operand {
                place.read_by(true, &mut out);
            }
        }
        if let Self::Defer { consumes, .. } = self {
            out.extend(consumes.iter().copied());
        }
        out
    }

    /// The blocks control may go to next.
    #[must_use]
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Self::Return | Self::Ended | Self::Unreachable => Vec::new(),
            Self::Goto(to) => vec![*to],
            Self::Yield { resume, cancel, .. } => vec![*resume, *cancel],
            Self::Branch { then, otherwise, .. } => vec![*then, *otherwise],
            Self::Defer { run, skip, .. } => vec![*run, *skip],
        }
    }

    /// The operands this reads.
    #[must_use]
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Branch { cond: o, .. } | Self::Yield { value: o, .. } => vec![o],
            _ => Vec::new(),
        }
    }

    /// The operands this reads, to rewrite.
    pub fn operands_mut(&mut self) -> Vec<&mut Operand> {
        match self {
            Self::Branch { cond: o, .. } | Self::Yield { value: o, .. } => vec![o],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum TerminatorKind {
    Return,
    /// Control never gets here, such as past the arms of an exhaustive `match`.
    Unreachable,
    /// `yield value`: suspends the coroutine with `value`; resuming it goes on at `resume`, and
    /// ending it while suspended here goes on at `cancel`, which runs what is pending here and
    /// ends in [`Self::Ended`] (spec 06, section 1).
    Yield {
        value: Operand,
        resume: BlockId,
        cancel: BlockId,
    },
    /// The frame, ended while suspended, stops without a result.
    Ended,
    Goto(BlockId),
    /// To `then` when the condition `cond` holds, to `otherwise` when it does not.
    Branch {
        cond: Operand,
        then: BlockId,
        otherwise: BlockId,
    },
    /// A deferred statement at an exit (spec 05, section 3): to `run`, which lowers it, when
    /// every linear local it consumes is `init` here, to `skip` when they were moved. The
    /// place states decide which, statically.
    Defer {
        consumes: Vec<Local>,
        run: BlockId,
        skip: BlockId,
    },
}
