//! Signatures in C. Values in memory are `u8 *`, and under Vun's convention a result in memory
//! is written through a last `u8 *` parameter. Under Vun's convention integers are `uN`; under
//! the C convention they are `iN` or `uN` as their LIR extension says, and records are `struct`s
//! passed and returned by value.

use vun_lir::{AbiType, Alias, Conv, Ext, Record, Signature};
use vun_origin::OriginId;

use super::Emitter;
use crate::ast::{Decl, Ident, Proto, Storage, Ty};
use crate::lack::{Lack, Missing};

/// A signature as C passes it.
pub(super) struct CSignature {
    pub(super) result: Ty,
    pub(super) params: Vec<Ty>,
}

impl Emitter<'_> {
    /// The C type passing `ty` under `conv`.
    pub(super) fn abi(&mut self, conv: Conv, ty: &AbiType, at: OriginId) -> Result<Ty, Lack> {
        match ty {
            &AbiType::Int(s, ext) if conv == Conv::C => self.int(s.bits(0), ext == Ext::Sign, at),
            &AbiType::Scalar(s) | &AbiType::Int(s, _) => self.scalar(s, at),
            AbiType::Address(Alias::Exclusive) => Ok(Ty::Restrict(Box::new(Ty::address()))),
            AbiType::Address(Alias::Any) | AbiType::Memory { .. } => Ok(Ty::address()),
            AbiType::Record(record) => Ok(Ty::Struct(self.record(record, at)?)),
        }
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
        if conv == Conv::Naked {
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
        Ok(CSignature { result, params })
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
        let params = c.params.into_iter().enumerate().map(|(i, ty)| Decl::new(ty, Ident::param(i))).collect();
        Ok(Proto { storage, result: c.result, name, params })
    }
}
