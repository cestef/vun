//! Signatures in C. Values in memory are `u8 *`, and under Vun's convention a result in memory
//! is written through a last `u8 *` parameter. Under Vun's convention integers are `uN`; under
//! the C convention they are `iN` or `uN` as their LIR extension says, and records are `struct`s
//! passed and returned by value. An integer twice a register wide is one `__int128` made of its
//! two halves, which C passes as the convention passes it.

use vun_lir::{AbiType, Alias, Conv, Ext, Record, Scalar, Signature};
use vun_origin::OriginId;
use vun_target::Attributes;

use super::Emitter;
use crate::ast::{Binary, Decl, Expr, Ident, Proto, Storage, Ty};
use vun_backend::{Lack, Missing};

/// A signature as C passes it.
pub(super) struct CSignature {
    pub(super) result: Ty,
    /// The type of each argument, variadic ones included.
    pub(super) params: Vec<Ty>,
    /// The parameter the variadic arguments start at, if any.
    pub(super) variadic: Option<usize>,
}

impl CSignature {
    /// The types of the parameters it declares, before any variadic ones.
    pub(super) fn declared(&self) -> &[Ty] {
        &self.params[..self.variadic.unwrap_or(self.params.len())]
    }

    /// A pointer to a function of this signature.
    pub(super) fn pointer(&self) -> Ty {
        let (result, params) = (Box::new(self.result.clone()), self.declared().to_vec());
        Ty::Function { result, params, variadic: self.variadic.is_some() }
    }
}

/// A value C declares as a signed integer, where Vun holds its bits unsigned: converted each
/// time it crosses a call, as argument, parameter, result or returned value.
pub(super) trait SignedC {
    /// Its scalar, when it passes as a signed C integer under `conv`.
    fn signed_c(&self, conv: Conv) -> Option<Scalar>;
}

impl SignedC for AbiType {
    fn signed_c(&self, conv: Conv) -> Option<Scalar> {
        match *self {
            AbiType::Int(s, Ext::Sign) if conv == Conv::C => Some(s),
            _ => None,
        }
    }
}

impl Emitter<'_> {
    /// The C type passing `ty` under `conv`.
    pub(super) fn abi(&mut self, conv: Conv, ty: &AbiType, at: OriginId) -> Result<Ty, Lack> {
        match ty {
            &AbiType::Int(s, ext) if conv == Conv::C => self.int(s.bits(0), ext == Ext::Sign, at),
            &AbiType::Scalar(s) | &AbiType::Int(s, _) => self.scalar(s, at),
            &AbiType::Wide { ext, .. } => Ok(Ty::Wide { signed: ext == Ext::Sign }),
            AbiType::Address(Alias::Exclusive) => Ok(Ty::Restrict(Box::new(Ty::address()))),
            AbiType::Address(Alias::Any) | AbiType::Memory { .. } => Ok(Ty::address()),
            AbiType::Record(record) => Ok(Ty::Struct(self.record(record, at)?)),
        }
    }

    /// The halves of the `__int128` `whole`, low first, each of `half`.
    pub(super) fn split(&self, whole: Expr, half: Scalar, at: OriginId) -> Result<[Expr; 2], Lack> {
        let (ty, whole) = (self.scalar(half, at)?, whole.cast(Ty::Wide { signed: false }));
        let high = Expr::binary(Binary::Shr, whole.clone(), Expr::small(u64::from(half.bits(0))));
        Ok([whole.cast(ty.clone()), high.cast(ty)])
    }

    /// The `__int128` of type `ty` whose halves, each of `half`, are `low` and `high`.
    pub(super) fn join(ty: Ty, [low, high]: [Expr; 2], half: Scalar) -> Expr {
        let wide = Ty::Wide { signed: false };
        let high = Expr::binary(Binary::Shl, high.cast(wide.clone()), Expr::small(u64::from(half.bits(0))));
        Expr::binary(Binary::Or, high, low.cast(wide)).cast(ty)
    }

    /// The `struct` of a record, declared once per layout.
    fn record(&mut self, record: &Record, at: OriginId) -> Result<Ident, Lack> {
        if let Some(i) = self.records.iter().position(|(r, _)| r == record) {
            return Ok(Ident::record(i));
        }
        let fields = record.fields.iter().map(|&(_, s)| self.scalar(s, at)).collect::<Result<_, _>>()?;
        self.records.push((record.clone(), fields));
        Ok(Ident::record(self.records.len() - 1))
    }

    pub(super) fn signature(&mut self, signature: &Signature, at: OriginId) -> Result<CSignature, Lack> {
        let conv = signature.conv;
        // A naked function is entered as its code, which GNU C writes, taking and giving nothing.
        let gnu = self.target.c.compiler.attributes == Attributes::Gnu;
        if conv == Conv::Naked && !(gnu && signature.params.is_empty() && signature.result.is_none()) {
            return Err(Missing::Naked.at(at));
        }
        let mut params =
            signature.params.iter().map(|p| self.abi(conv, p, at)).collect::<Result<Vec<_>, _>>()?;
        let result = match &signature.result {
            None => Ty::Void,
            Some(AbiType::Memory { .. }) => {
                params.push(Ty::address());
                Ty::Void
            }
            Some(result) => self.abi(conv, result, at)?,
        };
        Ok(CSignature { result, params, variadic: signature.variadic })
    }

    /// A declaration of a function named `name`, its parameters named by position.
    pub(super) fn proto(
        &mut self,
        storage: Storage,
        name: Ident,
        signature: &Signature,
        at: OriginId,
    ) -> Result<Proto, Lack> {
        let c = self.signature(signature, at)?;
        let params = c.declared().iter().enumerate().map(|(i, ty)| Decl::new(ty.clone(), Ident::param(i)));
        let (params, variadic) = (params.collect(), c.variadic.is_some());
        Ok(Proto { storage, result: c.result, name, params, variadic })
    }
}
