//! How a record travels, by its fields.

use vun_backend::Missing;
use vun_lir::{Record, Scalar};

use vun_asm_arm64::{Reg, V};

use super::{Carrier, HOMOGENEOUS, IN_REGISTERS, Part, WORD};

/// How a record travels, by its fields.
pub(super) enum Shape {
    /// Fields of one float type, each in a floating-point register.
    Homogeneous { count: usize, bytes: u64 },
    /// In general registers, a word each.
    Words(usize),
    /// Too large for registers: through a copy's address.
    Large,
}

impl Shape {
    pub(super) fn of(record: &Record) -> Result<Self, Missing> {
        let floats = record.fields.iter().map(|&(_, s)| is_float(s)).collect::<Result<Vec<_>, _>>()?;
        if let Some(&(_, first)) = record.fields.first()
            && floats.iter().all(|&f| f)
            && record.fields.len() <= HOMOGENEOUS
        {
            let bytes = if first == Scalar::F32 { 4 } else { 8 };
            let laid_out =
                record.fields.iter().enumerate().all(|(k, &(at, s))| s == first && at == k as u64 * bytes);
            if laid_out && record.size == record.fields.len() as u64 * bytes {
                return Ok(Self::Homogeneous { count: record.fields.len(), bytes });
            }
        }
        Ok(if record.size <= IN_REGISTERS {
            Self::Words(usize::try_from(record.size.div_ceil(WORD)).expect("two words"))
        } else {
            Self::Large
        })
    }

    /// Its parts, over `ints` and `fps`, the registers taken; `None` if too few are left.
    pub(super) fn parts(&self, record: &Record, ints: &mut &[Reg], fps: &mut &[V]) -> Option<Vec<Part>> {
        match *self {
            Self::Homogeneous { count, bytes } => {
                let (taken, rest) = fps.split_at_checked(count)?;
                *fps = rest;
                let parts = taken.iter().enumerate();
                Some(
                    parts
                        .map(|(k, &v)| Part { carrier: Carrier::Fp(v), offset: k as u64 * bytes, bytes })
                        .collect(),
                )
            }
            Self::Words(count) => {
                let (taken, rest) = ints.split_at_checked(count)?;
                *ints = rest;
                let parts = taken.iter().enumerate().map(|(k, &r)| {
                    let offset = k as u64 * WORD;
                    Part { carrier: Carrier::Int(r), offset, bytes: (record.size - offset).min(WORD) }
                });
                Some(parts.collect())
            }
            Self::Large => None,
        }
    }
}

/// A scalar's carrier class: whether it travels in a floating-point register.
pub(super) fn is_float(scalar: Scalar) -> Result<bool, Missing> {
    match scalar {
        Scalar::F16 => Err(Missing::Half),
        Scalar::I128 => Err(Missing::Int128),
        Scalar::F32 | Scalar::F64 => Ok(true),
        _ => Ok(false),
    }
}
