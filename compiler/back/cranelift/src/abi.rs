//! LIR's types and signatures as Cranelift's. Both of LIR's conventions are the target's own: a
//! signature's values are passed as the processor's convention places them, a result in memory
//! by the address after the parameters. A C record by value is placed from the classifier of
//! `vun-abi` (`plan`).

use cranelift_codegen::ir::{self, AbiParam, ArgumentPurpose, Type, types};
use cranelift_codegen::isa::CallConv;
use vun_abi::Variadic;
use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Ext, Scalar, Signature};

use crate::isa::Isa;
use crate::plan::{Param, Pass, Placed, Returned, SLOT, Skip};

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
            Scalar::V128 => return Err(Missing::Vector),
        })
    }

    /// A struct argument of `size` bytes, a multiple of a word.
    fn struct_argument(size: u64, ptr: AbiParam) -> Result<AbiParam, Missing> {
        let size = u32::try_from(size).map_err(|_| Missing::Record)?;
        Ok(AbiParam { purpose: ArgumentPurpose::StructArgument(size), ..ptr })
    }

    /// The parameter that carries nothing in place of `skip`: a word of the stack is a struct
    /// argument before one, else an integer, which reaches the stack as the registers are taken.
    fn skipped(skip: Skip, structs: bool, ptr: AbiParam) -> Result<AbiParam, Missing> {
        Ok(match skip {
            Skip::Float => AbiParam::new(types::F64),
            Skip::Stack if structs => Self::struct_argument(SLOT, ptr)?,
            Skip::Int | Skip::Stack => AbiParam::new(types::I64),
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
            AbiType::Record(_) if conv == Conv::Vun => vec![self.param(Scalar::Ptr, None)?],
            AbiType::Wide { .. } if conv == Conv::C => return Err(Missing::Int128),
            AbiType::Wide { limb, limbs, .. } => {
                (0..*limbs).map(|_| self.param(*limb, None)).collect::<Result<_, _>>()?
            }
            AbiType::Record(_) => return Err(Missing::Record),
        })
    }

    /// How a call of `signature` passes its values and gives its result, as the native backend
    /// passes them: the address of a result in memory last, as one more argument, but for a C
    /// record's, which the convention places.
    ///
    /// # Errors
    /// Returns what Cranelift does not express of it: variadic arguments, a record the convention
    /// places where Cranelift would not, and under the C convention a wide integer, which that
    /// convention places by its own rules.
    pub(crate) fn plan(&self, signature: &Signature) -> Result<Plan, Missing> {
        let mut out = ir::Signature::new(self.target.default_call_conv());
        // A naked function is entered, never called: it passes nothing.
        if signature.conv == Conv::Naked {
            return Ok(Plan { signature: out, params: Vec::new(), returned: Returned::Direct });
        }
        if let Some(fixed) = signature.variadic {
            self.variadic(&signature.params[fixed..])?;
        }
        let conv = signature.conv;
        let records =
            signature.params.iter().chain(&signature.result).any(|t| matches!(t, AbiType::Record(_)));
        let placed =
            if conv == Conv::C && records { self.place(signature)? } else { Placed::plain(signature) };
        let ptr = self.param(Scalar::Ptr, None)?;
        if placed.returned == Returned::Sret {
            out.params.push(AbiParam { purpose: ArgumentPurpose::StructReturn, ..ptr });
        }
        for (ty, Param { skips, pass }) in signature.params.iter().zip(&placed.params) {
            for skip in skips {
                out.params.push(Self::skipped(*skip, matches!(pass, Pass::Struct(_)), ptr)?);
            }
            match pass {
                Pass::Values(_) => out.params.extend(self.parts(ty, conv)?),
                Pass::Parts(parts) => {
                    out.params.extend(parts.lanes.iter().map(|lane| AbiParam::new(lane.ty)));
                }
                Pass::Struct(layout) => out.params.push(Self::struct_argument(layout.words(), ptr)?),
                Pass::Words(layout) => {
                    let words = usize::try_from(layout.words() / SLOT).map_err(|_| Missing::Record)?;
                    out.params.extend(std::iter::repeat_n(AbiParam::new(types::I64), words));
                }
                Pass::Copy(_) => out.params.push(ptr),
            }
        }
        let returned = match placed.returned {
            Returned::Direct | Returned::Through => {
                let result = signature.result.as_ref().map(|ty| self.parts(ty, conv)).transpose()?;
                if signature.through() {
                    out.params.push(ptr);
                    Returned::Through
                } else {
                    out.returns.extend(result.into_iter().flatten());
                    Returned::Direct
                }
            }
            Returned::Parts(parts) => {
                out.returns.extend(parts.lanes.iter().map(|lane| AbiParam::new(lane.ty)));
                Returned::Parts(parts)
            }
            Returned::Sret => Returned::Sret,
        };
        Ok(Plan { signature: out, params: placed.params, returned })
    }
}

impl Isa {
    /// How the body of a function with an entry is passed to: every value as Cranelift's tail
    /// convention places it, a C record by the address of its memory and a result in memory by an
    /// address, which the entry (`Cx::entry`) decodes the target's convention to and from.
    ///
    /// # Errors
    /// Returns what Cranelift does not express of the signature.
    pub(crate) fn body_plan(&self, signature: &Signature) -> Result<Plan, Missing> {
        let vun = Signature { conv: Conv::Vun, convention: None, ..signature.clone() };
        let mut plan = self.plan(&vun)?;
        plan.signature.call_conv = CallConv::Tail;
        Ok(plan)
    }
}

/// A signature as Cranelift's, and how its values reach it.
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub(crate) signature: ir::Signature,
    /// How each parameter of the LIR signature is passed.
    pub(crate) params: Vec<Param>,
    pub(crate) returned: Returned,
}
