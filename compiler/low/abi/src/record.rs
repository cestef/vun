//! How a record travels in registers, by its fields: the parts it is cut into, each with the class
//! of register it takes.

use vun_lir::{Record, Scalar};

use crate::Unpassable;
use crate::convention::{POINTER_BITS, Records, Rules, WORD};

/// A class of registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Int,
    Float,
}

/// Bytes of a record for one register of a class: their offset and their count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) class: Class,
    pub(crate) offset: u64,
    pub(crate) bytes: u64,
}

/// The bytes an eightbyte of floats may hold: a `f32`, two, or a `f64`, each moving between
/// memory and its register in one instruction.
const FLOAT_PARTS: [u64; 2] = [4, 8];

impl Class {
    /// The class of register a scalar travels in.
    pub(crate) fn of(scalar: Scalar) -> Result<Self, Unpassable> {
        match scalar {
            Scalar::F16 => Err(Unpassable::Half),
            Scalar::I128 => Err(Unpassable::Int128),
            Scalar::F32 | Scalar::F64 => Ok(Self::Float),
            Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::I64 | Scalar::Ptr => Ok(Self::Int),
        }
    }
}

impl Rules {
    /// The pieces `record` travels in registers as, or `None` when it is too large to.
    pub(crate) fn pieces(&self, record: &Record) -> Result<Option<Vec<Piece>>, Unpassable> {
        if record.size > self.in_registers {
            return Ok(None);
        }
        let classes = record.fields.iter().map(|&(_, s)| Class::of(s)).collect::<Result<Vec<_>, _>>()?;
        match self.records {
            Records::Eightbytes => Self::eightbytes(record, &classes).map(Some),
            Records::Homogeneous { max } => {
                Ok(Some(Self::homogeneous(record, &classes, max).unwrap_or_else(|| Self::words(record))))
            }
        }
    }

    /// Each eightbyte in a floating-point register when every field in it is a float.
    fn eightbytes(record: &Record, classes: &[Class]) -> Result<Vec<Piece>, Unpassable> {
        let starts = (0..record.size.div_ceil(WORD)).map(|k| k * WORD);
        starts
            .map(|offset| {
                let inside = record
                    .fields
                    .iter()
                    .zip(classes)
                    .filter(|((at, _), _)| (offset..offset + WORD).contains(at));
                let mut classes = inside.map(|(_, &c)| c).peekable();
                let float = classes.peek().is_some() && classes.all(|c| c == Class::Float);
                let bytes = (record.size - offset).min(WORD);
                if float && !FLOAT_PARTS.contains(&bytes) {
                    return Err(Unpassable::FloatPart(bytes));
                }
                Ok(Piece { class: if float { Class::Float } else { Class::Int }, offset, bytes })
            })
            .collect()
    }

    /// A field per floating-point register, when the record is up to `max` floats of one type laid
    /// out one after another.
    fn homogeneous(record: &Record, classes: &[Class], max: usize) -> Option<Vec<Piece>> {
        let &(_, first) = record.fields.first()?;
        let bytes = first.bytes(POINTER_BITS);
        let dense =
            record.fields.iter().enumerate().all(|(k, &(at, s))| s == first && at == k as u64 * bytes);
        let count = record.fields.len();
        let homogeneous = classes.iter().all(|&c| c == Class::Float)
            && count <= max
            && dense
            && record.size == count as u64 * bytes;
        homogeneous.then(|| {
            (0..count as u64).map(|k| Piece { class: Class::Float, offset: k * bytes, bytes }).collect()
        })
    }

    /// A word per general register.
    fn words(record: &Record) -> Vec<Piece> {
        let starts = (0..record.size.div_ceil(WORD)).map(|k| k * WORD);
        starts
            .map(|offset| Piece { class: Class::Int, offset, bytes: (record.size - offset).min(WORD) })
            .collect()
    }
}
