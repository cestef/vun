//! Signatures: each run-time parameter as its representation passes it. Under Vun's convention a
//! value in memory is passed by address, a result in memory written through an address; under
//! the C convention (`#[abi.c]`) such a value is a record of the scalars C sees in it.

use vun_desugar::Access;
use vun_lir::{AbiType, Alias, Conv, Ext, Record, Scalar, Signature};
use vun_origin::OriginId;
use vun_primitives::abi::Abi;
use vun_sema::layout::Parts;
use vun_sema::member::Members;
use vun_sema_ty::{Closure, Ty, TyKind};

use crate::lowering::Lowering;
use crate::repr::{Repr, Types};
use crate::unlowered::{Form, Unlowered};

impl Lowering<'_, '_> {
    /// The LIR signature of a function of type `signature`, called under `abi`.
    pub(crate) fn signature_of(
        &self,
        signature: &vun_sema_ty::Signature,
        abi: Option<Abi>,
        at: OriginId,
    ) -> Result<Signature, Unlowered> {
        self.signature(&signature.params, signature.result, abi, at)
    }

    /// How a value of type `ty` is passed under `conv`; `None` for a value of size 0.
    pub(crate) fn passed(&self, ty: Ty, conv: Conv, at: OriginId) -> Result<Option<AbiType>, Unlowered> {
        Ok(match self.types.repr(ty, at)? {
            Repr::Zero => None,
            // An integer extends by its range's sign; a tag or a truth value stored as one is
            // bits, read unsigned.
            Repr::Scalar(s) if s.is_int() => {
                let signed = self.types.int(ty).is_some_and(|(_, signed)| signed);
                Some(AbiType::Int(s, if signed { Ext::Sign } else { Ext::Zero }))
            }
            // An exclusive borrow's address reaches what nothing else does while it lives.
            Repr::Scalar(Scalar::Ptr) => {
                let exclusive = ty.borrowing().is_some_and(|(mode, ..)| mode.access() == Access::Exclusive);
                Some(AbiType::Address(if exclusive { Alias::Exclusive } else { Alias::Any }))
            }
            Repr::Scalar(s) => Some(AbiType::Scalar(s)),
            Repr::Memory(_) if conv == Conv::C => Some(AbiType::Record(self.types.record(ty, at)?)),
            Repr::Memory(layout) => Some(AbiType::Memory { size: layout.size, align: layout.align }),
        })
    }

    /// The signature of a coroutine's `resume` (giving its step, of type `step`) or `end`
    /// (`None`): the frame's address, which nothing else reaches while it runs.
    pub(crate) fn frame_signature(&self, step: Option<Ty>, at: OriginId) -> Result<Signature, Unlowered> {
        let result = step.map(|step| self.passed(step, Conv::Vun, at)).transpose()?.flatten();
        Ok(Signature {
            conv: Conv::Vun,
            params: vec![AbiType::Address(Alias::Exclusive)],
            result,
            variadic: None,
        })
    }

    /// The LIR signature of parameters of the types `params`, their result of type `result`.
    /// A pack passes the tuple of its arguments; under the C convention, each of its items as a
    /// variadic argument of C.
    pub(crate) fn signature(
        &self,
        params: &[Ty],
        result: Ty,
        abi: Option<Abi>,
        at: OriginId,
    ) -> Result<Signature, Unlowered> {
        let conv = match abi {
            Some(Abi::c) => Conv::C,
            Some(Abi::naked) => Conv::Naked,
            _ => Conv::Vun,
        };
        let (mut passed, mut variadic) = (Vec::new(), None);
        for &param in params {
            match param.packed() {
                Some(items) if conv == Conv::C => {
                    variadic = Some(passed.len());
                    for item in self.types.parts(items).ok_or(Unlowered::unsupported(at, Form::Callee))? {
                        passed.extend(self.passed(item, conv, at)?);
                    }
                }
                packed => passed.extend(self.passed(packed.unwrap_or(param), conv, at)?),
            }
        }
        let result = self.passed(result, conv, at)?;
        Ok(Signature { conv, params: passed, result, variadic })
    }
}

impl Types<'_, '_> {
    /// `ty` as a record of the scalars at their offsets, as C lays out a struct.
    pub(crate) fn record(self, ty: Ty, at: OriginId) -> Result<Record, Unlowered> {
        let layout = self.shape(ty, at)?.layout;
        let mut fields = Vec::new();
        self.leaves(ty, 0, &mut fields, at)?;
        Ok(Record { size: layout.size, align: layout.align, fields })
    }

    /// The scalars of a value of type `ty` placed at `base`, in order.
    fn leaves(self, ty: Ty, base: u64, out: &mut Vec<(u64, Scalar)>, at: OriginId) -> Result<(), Unlowered> {
        let shape = self.shape(ty, at)?;
        if let Repr::Scalar(s) = self.repr(ty, at)? {
            out.push((base, s));
            return Ok(());
        }
        // A borrow of a slice: its address, then its length.
        if ty.borrowed().is_some_and(|t| t.slice_items().is_some()) {
            let word = self.word;
            out.extend([(base, Scalar::Ptr), (base + word.bytes(self.pointer_bits()), word)]);
            return Ok(());
        }
        match &shape.parts {
            Parts::Sequence(offsets) => {
                let parts = self.parts(ty).ok_or(Unlowered::unsupported(at, Form::Conversion))?;
                for (part, offset) in parts.into_iter().zip(offsets) {
                    self.leaves(part, base + offset, out, at)?;
                }
                Ok(())
            }
            Parts::Array { stride, len } => {
                let item = ty
                    .as_array()
                    .map(|(_, item)| item)
                    .ok_or(Unlowered::unsupported(at, Form::Conversion))?;
                for i in 0..*len {
                    self.leaves(item, base + i * stride, out, at)?;
                }
                Ok(())
            }
            Parts::Whole | Parts::Bits(_) | Parts::Enum { .. } => {
                Err(Unlowered::unsupported(at, Form::Conversion))
            }
        }
    }

    /// The types of the parts of a value laid out as a sequence, in order: a struct's fields, a
    /// tuple's items, a record's fields, a closure's captures.
    pub(crate) fn parts(self, ty: Ty) -> Option<Vec<Ty>> {
        match &*ty {
            TyKind::Tuple(items) => Some(items.clone()),
            TyKind::Record(fields) => Some(fields.iter().map(|(_, t)| *t).collect()),
            TyKind::Closure(Closure { captures, .. }) => Some(captures.iter().map(|(_, t)| *t).collect()),
            TyKind::Nominal(_) => {
                let members = self.cx.get::<Members>(&ty)?;
                members.fields.iter().map(|f| f.ty.as_ref().ok().copied()).collect()
            }
            TyKind::Primitive(..) | TyKind::Frame(_) | TyKind::Param(_) => None,
        }
    }
}
