//! Each function of a program lowered on its own (spec 13, section 3): its LIR, tidied, and
//! what its ids name, in tables of its own. A function of a library lowers once for every
//! program reaching it, its memo shared with them (spec 13, section 2); the program places the
//! ids when it is assembled (`assemble`).

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Function, Linkage, Reloc, Site};
use vun_lir_passes::Tidy;
use vun_mir::{Body, Concrete};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Nominal, Ty, TyKind};

use crate::func::{FnLower, Mode};
use crate::lowering::Lowering;
use crate::unchecked::Unchecked;
use crate::unlowered::{Form, Unlowered, Why};

/// A function of a program, as lowering makes it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Piece {
    /// The instantiation `f`, or a coroutine's `resume`; with `end`, a coroutine's `end`. The
    /// checks of unsafe code apply in it when `checked`.
    Function { f: Instantiation, end: bool, checked: bool },
    /// The constructor of the variant `name` of the enum `nominal`, taken as a value.
    Constructor { nominal: Nominal, name: Symbol },
}

/// A function's LIR, tidied, and what its ids name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Lir {
    pub function: Function,
    pub refs: Refs,
}

/// What a function's ids name, by position: its `FuncId`s, `ImportId`s, `DataId`s and trap
/// sites.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Refs {
    pub functions: Vec<FuncRef>,
    /// The declarations the linker provides, one per symbol.
    pub imports: Vec<DeclRef>,
    /// Each after the data its bytes hold the addresses of.
    pub data: Vec<Datum>,
    pub sites: Vec<Site>,
}

/// A function another names.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum FuncRef {
    /// An instantiation, or a coroutine's `resume`.
    Function(Instantiation),
    /// A coroutine's `end`.
    End(Instantiation),
    /// The constructor of a variant taken as a value.
    Constructor(Ty, Symbol),
}

impl std::fmt::Display for FuncRef {
    /// Where the function is written and which of its `$` parameters are bound, for logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, i) = match self {
            Self::Function(i) => ("function", i),
            Self::End(i) => ("end", i),
            Self::Constructor(ty, name) => return write!(f, "constructor {name} of {ty}"),
        };
        let bound: String = i.args.iter().map(|a| if a.is_some() { 'x' } else { '_' }).collect();
        write!(f, "{kind} {}#{} bound [{bound}]", i.f.literal.file.path, i.f.literal.expr.id.index())
    }
}

/// A data object a function names.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Datum {
    pub bytes: Bytes,
    pub align: u64,
    pub kind: DatumKind,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum DatumKind {
    /// Read-only, made once per content.
    Constant,
    /// The object of a top-level binding, made once per binding; `writable` for a `mut` one.
    Global { decl: DeclRef, writable: bool },
}

/// A value's bytes: the addresses to write into them, and the trap sites whose ids they hold.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Bytes {
    pub bytes: Vec<u8>,
    pub relocs: Vec<Reloc>,
    pub sites: Vec<SiteAt>,
}

/// A trap site's id in a value's bytes: at `offset`, `width` bytes wide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct SiteAt {
    pub offset: u64,
    pub width: u64,
    pub site: u32,
}

/// Each function of a program lowered on its own, and tidied.
pub struct FunctionLir;

impl Query for FunctionLir {
    const NAME: &'static str = "function_lir";
    type Key = Piece;
    type Value = Arc<Result<Lir, Unlowered>>;

    fn compute(cx: &Cx<'_>, piece: &Piece) -> Self::Value {
        Arc::new(piece.lowered(cx))
    }
}

impl Piece {
    /// The function `f`, or with `end` its `end`, with the checks but where they run.
    pub(crate) fn of(f: &Instantiation, end: bool, unchecked: &Unchecked) -> Self {
        Piece::Function { f: f.clone(), end, checked: !unchecked.contains(f) }
    }

    /// The instantiation it lowers, if any.
    pub(crate) fn instantiation(&self) -> Option<&Instantiation> {
        match self {
            Piece::Function { f, .. } => Some(f),
            Piece::Constructor { .. } => None,
        }
    }

    /// The function's LIR, tidied, and what its ids name.
    fn lowered(&self, cx: &Cx<'_>) -> Result<Lir, Unlowered> {
        let (mut function, l) = match self {
            Piece::Function { f, end, checked } => {
                let mut l = Lowering::new(cx, &f.f.literal.file, *checked)?;
                l.cover(f);
                let body = cx.get::<Concrete>(f).map_err(|u| Unlowered::new(u.at(), Why::Unready(u)))?;
                let function = match (body.suspends(), end) {
                    (true, false) => FnLower::coroutine(&mut l, f, &body, Mode::Resume)?,
                    (true, true) => FnLower::coroutine(&mut l, f, &body, Mode::End)?,
                    (false, false) => FnLower::lower(&mut l, f, &body, Linkage::Internal)?,
                    (false, true) => return Err(Unlowered::unsupported(body.origin, Form::Frame)),
                };
                (function, l)
            }
            Piece::Constructor { nominal, name } => {
                let mut l = Lowering::new(cx, &nominal.literal.file, true)?;
                let ty = TyKind::Nominal(nominal.clone()).intern();
                let at = nominal.literal.expr.origin;
                let body = Body::constructor(cx, ty, *name).ok_or(Unlowered::new(at, Why::NoLayout(ty)))?;
                let params = body.params.clone();
                let function =
                    FnLower::function(&mut l, &body, &params, None, Linkage::Internal, Arc::default(), true)?;
                (function, l)
            }
        };
        // Tidying reads no other function: it is done once per function, with its memo.
        function.tidy(l.types.pointer_bits());
        Ok(Lir { function, refs: l.refs })
    }
}
