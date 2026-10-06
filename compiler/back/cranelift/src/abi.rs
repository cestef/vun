//! LIR's types and signatures as Cranelift's. Both of LIR's conventions are the target's own: a
//! signature's values are passed as the processor's convention places them, a result in memory
//! by the address after the parameters.

use cranelift_codegen::ir::{self, AbiParam, Type, types};
use vun_abi::Variadic;
use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Ext, Scalar, Signature};

use crate::isa::Isa;

impl Isa {
    /// The type of a LIR scalar.
    pub(crate) fn ty(&self, scalar: Scalar) -> Result<Type, Missing> {
        Ok(match scalar {
            Scalar::I8 => types::I8,
            Scalar::I16 => types::I16,
            Scalar::I32 => types::I32,
            Scalar::I64 => types::I64,
            Scalar::F32 => types::F32,
            Scalar::F64 => types::F64,
            Scalar::Ptr => self.target.pointer_type(),
            Scalar::I128 => return Err(Missing::Int128),
            Scalar::F16 => return Err(Missing::Half),
        })
    }

    fn param(&self, scalar: Scalar, ext: Option<Ext>) -> Result<AbiParam, Missing> {
        let param = AbiParam::new(self.ty(scalar)?);
        Ok(match ext {
            Some(Ext::Zero) => param.uext(),
            Some(Ext::Sign) => param.sext(),
            None => param,
        })
    }

    /// Whether a call passes `variadic` arguments as Cranelift passes arguments: as fixed ones,
    /// which the convention of AAPCS64 does. System V does too but for a call stating how many
    /// floating-point registers carry arguments, which Cranelift does not, so a float among them
    /// is refused; without one, the callee saves every register or none, which an argument in an
    /// integer register does not mind.
    fn variadic(&self, variadic: &[AbiType]) -> Result<(), Missing> {
        let floats = variadic.iter().any(|ty| matches!(ty, AbiType::Scalar(s) if s.is_float()));
        match self.rules.map(|rules| rules.variadic) {
            Some(Variadic::Fixed) => Ok(()),
            Some(Variadic::FloatCount) if !floats => Ok(()),
            _ => Err(Missing::Variadic),
        }
    }

    /// The values a parameter or result of type `ty` is passed as, under `conv`.
    fn parts(&self, ty: &AbiType, conv: Conv) -> Result<Vec<AbiParam>, Missing> {
        Ok(match ty {
            AbiType::Scalar(s) => vec![self.param(*s, None)?],
            AbiType::Int(s, ext) => vec![self.param(*s, Some(*ext))?],
            AbiType::Address(_) | AbiType::Memory { .. } => vec![self.param(Scalar::Ptr, None)?],
            AbiType::Wide { .. } if conv == Conv::C => return Err(Missing::Int128),
            AbiType::Wide { limb, limbs, .. } => {
                (0..*limbs).map(|_| self.param(*limb, None)).collect::<Result<_, _>>()?
            }
            AbiType::Record(_) => return Err(Missing::Record),
        })
    }

    /// How a call of `signature` passes its values and gives its result, as the native backend
    /// passes them: the address of a result in memory last, as one more argument.
    ///
    /// # Errors
    /// Returns what Cranelift does not express of it: variadic arguments, a record, and under
    /// the C convention a wide integer, which that convention places by its own rules.
    pub(crate) fn signature(&self, signature: &Signature) -> Result<ir::Signature, Missing> {
        let mut out = ir::Signature::new(self.target.default_call_conv());
        // A naked function is entered, never called: it passes nothing.
        if signature.conv == Conv::Naked {
            return Ok(out);
        }
        if let Some(fixed) = signature.variadic {
            self.variadic(&signature.params[fixed..])?;
        }
        let conv = signature.conv;
        for ty in &signature.params {
            out.params.extend(self.parts(ty, conv)?);
        }
        let result = signature.result.as_ref().map(|ty| self.parts(ty, conv)).transpose()?;
        if signature.through() {
            out.params.push(self.param(Scalar::Ptr, None)?);
        } else {
            out.returns.extend(result.into_iter().flatten());
        }
        Ok(out)
    }
}
