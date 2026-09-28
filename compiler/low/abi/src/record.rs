//! Classes of registers, and how a record travels in registers by its fields: the parts it is cut
//! into, each with the class of register it takes.

use vun_lir::{Record, Scalar};

use crate::{Floats, Records, Rules, Unpassable};

/// A class of registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Int,
    Float,
}

/// Bytes of a value for one register of a class: their offset and their count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) class: Class,
    pub(crate) offset: u64,
    pub(crate) bytes: u64,
}

/// The bytes an eightbyte of floats may hold: a `f32`, two, or a `f64`, each moving between
/// memory and its register in one instruction.
const FLOAT_PARTS: [u64; 2] = [4, 8];

impl Piece {
    /// The `count` limbs of `limb` bytes of an integer, a general register each.
    pub(crate) fn limbs(limb: u64, count: u8) -> Vec<Self> {
        (0..u64::from(count)).map(|k| Self { class: Class::Int, offset: k * limb, bytes: limb }).collect()
    }
}

impl Rules {
    /// The width of an address.
    pub(crate) fn pointer_bits(&self) -> u32 {
        u32::try_from(self.word * 8).expect("a word of a few bytes")
    }

    /// The class of register a scalar travels in as one value.
    pub(crate) fn class(&self, scalar: Scalar) -> Result<Class, Unpassable> {
        let class = self.field(scalar)?;
        let bytes = scalar.bytes(self.pointer_bits());
        match class {
            Class::Int if bytes > self.word => Err(Unpassable::Wider(bytes)),
            _ => Ok(class),
        }
    }

    /// Whether a scalar fits a register of its class: legalization splits a wider one into
    /// halves before a call passes it.
    #[must_use]
    pub fn fits(&self, scalar: Scalar) -> bool {
        !matches!(self.class(scalar), Err(Unpassable::Wider(_)))
    }

    /// The class of register a field of a record travelling in registers takes.
    fn field(&self, scalar: Scalar) -> Result<Class, Unpassable> {
        match scalar {
            Scalar::F16 => Err(Unpassable::Half),
            Scalar::F32 | Scalar::F64 if self.floats != Floats::Core => Ok(Class::Float),
            _ => Ok(Class::Int),
        }
    }

    /// The pieces `record` travels in registers as, or `None` when it is larger than `limit`
    /// and no homogeneous aggregate.
    pub(crate) fn pieces(&self, record: &Record, limit: u64) -> Result<Option<Vec<Piece>>, Unpassable> {
        let classes = record.fields.iter().map(|&(_, s)| self.field(s)).collect::<Result<Vec<_>, _>>()?;
        if let Records::Homogeneous { max } = self.records
            && let Some(pieces) = self.homogeneous(record, &classes, max)
        {
            return Ok(Some(pieces));
        }
        if record.size > limit {
            return Ok(None);
        }
        match self.records {
            Records::Eightbytes => self.eightbytes(record, &classes).map(Some),
            Records::Homogeneous { .. } => Ok(Some(self.words(record))),
        }
    }

    /// The offsets of the words of `record`.
    fn starts(&self, record: &Record) -> impl Iterator<Item = u64> {
        let word = self.word;
        (0..record.size.div_ceil(word)).map(move |k| k * word)
    }

    /// Each eightbyte in a floating-point register when every field in it is a float.
    fn eightbytes(&self, record: &Record, classes: &[Class]) -> Result<Vec<Piece>, Unpassable> {
        self.starts(record)
            .map(|offset| {
                let inside = record
                    .fields
                    .iter()
                    .zip(classes)
                    .filter(|((at, _), _)| (offset..offset + self.word).contains(at));
                let mut classes = inside.map(|(_, &c)| c).peekable();
                let float = classes.peek().is_some() && classes.all(|c| c == Class::Float);
                let bytes = (record.size - offset).min(self.word);
                if float && !FLOAT_PARTS.contains(&bytes) {
                    return Err(Unpassable::FloatPart(bytes));
                }
                Ok(Piece { class: if float { Class::Float } else { Class::Int }, offset, bytes })
            })
            .collect()
    }

    /// A field per floating-point register, when the record is up to `max` floats of one type laid
    /// out one after another.
    fn homogeneous(&self, record: &Record, classes: &[Class], max: usize) -> Option<Vec<Piece>> {
        let &(_, first) = record.fields.first()?;
        let bytes = first.bytes(self.pointer_bits());
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
    fn words(&self, record: &Record) -> Vec<Piece> {
        let piece =
            |offset| Piece { class: Class::Int, offset, bytes: (record.size - offset).min(self.word) };
        self.starts(record).map(piece).collect()
    }
}
