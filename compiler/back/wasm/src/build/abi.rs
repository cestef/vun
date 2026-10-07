//! How a LIR signature is passed in wasm (spec 13, section 8.4): clang's wasm32 C convention,
//! which Vun's follows too. Scalars are values; a record of one scalar is that scalar; another
//! record is the address of a copy the caller makes; a result in memory is written through an
//! address passed first; narrow integers are extended as their signature says; variadic
//! arguments are written into a buffer whose address is passed last (`varargs`).

use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Ext, Record, Scalar, Signature};

use super::varargs::Varargs;
use crate::module::{FuncType, ValType};

/// How one LIR value of a signature's parameters is passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    /// As itself; a narrow integer of this many bits extended as it says.
    Value(ValType, Option<(u32, Ext)>),
    /// The address of a record of one scalar, passed as the scalar.
    Scalar(One),
    /// The address of a record, passed as the address of a copy the caller makes.
    Copy { size: u64, align: u64 },
}

/// How a result is given back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Given {
    None,
    Value(ValType, Option<(u32, Ext)>),
    /// A record of one scalar, returned as it, which the body writes through an address.
    Scalar(One),
    /// Written through the address passed first.
    Through,
}

/// A record of one scalar, passed and returned as the scalar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct One {
    pub(super) ty: Scalar,
    pub(super) held: ValType,
    pub(super) offset: u64,
    pub(super) size: u64,
    pub(super) align: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Passing {
    /// For each value of the LIR signature's fixed parameters.
    pub(super) parts: Vec<Part>,
    /// How the variadic arguments are passed, if the signature has them.
    pub(super) varargs: Option<Varargs>,
    pub(super) given: Given,
    pub(super) ty: FuncType,
}

/// The value type holding a LIR scalar; integers up to 32 bits and addresses are `i32`.
pub(super) fn val(ty: Scalar) -> Result<ValType, Missing> {
    match ty {
        Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::Ptr => Ok(ValType::I32),
        Scalar::I64 => Ok(ValType::I64),
        Scalar::F32 => Ok(ValType::F32),
        Scalar::F64 => Ok(ValType::F64),
        Scalar::I128 => Err(Missing::Int128),
        Scalar::F16 => Err(Missing::Half),
        Scalar::V128 => Ok(ValType::V128),
    }
}

/// The width of an integer held narrower than its value type, if it is.
pub(super) fn narrow(ty: Scalar) -> Option<u32> {
    matches!(ty, Scalar::I8 | Scalar::I16).then(|| ty.bits(0))
}

impl Passing {
    /// Whether a call passing this can replace its caller's frame: its arguments and result are
    /// values, not copies in the frame, spills or memory.
    pub(super) fn replaces(&self) -> bool {
        self.varargs.is_none()
            && !self.through()
            && matches!(self.given, Given::Value(..) | Given::None)
            && self.parts.iter().all(|part| !matches!(part, Part::Copy { .. }))
    }

    pub(super) fn of(signature: &Signature) -> Result<Self, Missing> {
        if signature.conv == Conv::Naked {
            return Err(Missing::Naked);
        }
        let fixed = signature.variadic.unwrap_or(signature.params.len());
        let (params, variadic) = signature.params.split_at(fixed);
        let varargs = signature.variadic.map(|_| Varargs::of(variadic)).transpose()?;
        let mut parts = Vec::new();
        for p in params {
            match p {
                &AbiType::Wide { limbs, .. } => {
                    parts.extend(std::iter::repeat_n(Part::Value(ValType::I64, None), usize::from(limbs)));
                }
                other => parts.push(Self::part(other)?),
            }
        }
        let given = match &signature.result {
            None => Given::None,
            Some(AbiType::Record(r)) => Self::one(r)?.map_or(Given::Through, Given::Scalar),
            Some(AbiType::Memory { .. } | AbiType::Wide { .. }) => Given::Through,
            Some(other) => match Self::part(other)? {
                Part::Value(ty, narrow) => Given::Value(ty, narrow),
                Part::Scalar { .. } | Part::Copy { .. } => Given::Through,
            },
        };
        let through = (given == Given::Through).then_some(ValType::I32);
        // The buffer of variadic arguments is passed after the fixed ones.
        let buffer = varargs.as_ref().map(|_| ValType::I32);
        let params = through.into_iter().chain(parts.iter().map(|p| p.passed())).chain(buffer).collect();
        let results = match given {
            Given::Value(ty, _) => vec![ty],
            Given::Scalar(one) => vec![one.held],
            Given::None | Given::Through => Vec::new(),
        };
        Ok(Self { parts, varargs, given, ty: FuncType { params, results } })
    }

    fn part(p: &AbiType) -> Result<Part, Missing> {
        Ok(match p {
            AbiType::Scalar(s) => Part::Value(val(*s)?, None),
            AbiType::Int(s, ext) => Part::Value(val(*s)?, narrow(*s).map(|bits| (bits, *ext))),
            AbiType::Address(_) | AbiType::Memory { .. } => Part::Value(ValType::I32, None),
            AbiType::Record(r) => {
                Self::one(r)?.map_or(Part::Copy { size: r.size, align: r.align }, Part::Scalar)
            }
            AbiType::Wide { .. } => Part::Value(ValType::I64, None),
        })
    }

    /// The record as its one scalar, if it holds one.
    fn one(r: &Record) -> Result<Option<One>, Missing> {
        match r.fields.as_slice() {
            &[(offset, ty)] => Ok(Some(One { ty, held: val(ty)?, offset, size: r.size, align: r.align })),
            _ => Ok(None),
        }
    }

    /// Whether the address a result is written through is passed, as the first value.
    pub(super) fn through(&self) -> bool {
        self.given == Given::Through
    }
}

impl Part {
    /// The value type it is passed as.
    fn passed(self) -> ValType {
        match self {
            Self::Value(ty, _) => ty,
            Self::Scalar(one) => one.held,
            Self::Copy { .. } => ValType::I32,
        }
    }
}
