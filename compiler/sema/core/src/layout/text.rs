//! The layout text format, as `vun debug dump layout` prints it: a type's size and alignment, how an
//! enum tells its variants, where each part is, and its unused patterns.

use std::fmt;

use vun_intern::Symbol;

use super::{Layout, Niche, Offset, Parts, Shape, Tag};
use crate::member::MemberTable;

/// A shape as text, its parts named by the members of its type, when it has any.
pub struct Text<'s> {
    shape: &'s Shape,
    members: Option<&'s MemberTable>,
}

impl Shape {
    /// This shape as text, its fields and variants named by `members`.
    #[must_use]
    pub fn text<'s>(&'s self, members: Option<&'s MemberTable>) -> Text<'s> {
        Text { shape: self, members }
    }
}

impl fmt::Display for Niche {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} unused from {} in the {}-bit integer at {}",
            self.count,
            self.start,
            Text::bits(self.bytes),
            self.offset
        )
    }
}

impl Text<'_> {
    fn bits(bytes: u64) -> u64 {
        bytes * u64::from(u8::BITS)
    }

    /// Offsets as a list: `at 0, 4`.
    fn offsets(f: &mut fmt::Formatter<'_>, offsets: &[u64]) -> fmt::Result {
        for (i, offset) in offsets.iter().enumerate() {
            write!(f, "{}{offset}", if i == 0 { " at " } else { ", " })?;
        }
        Ok(())
    }

    /// The name of part `i`: a field's name, or its position.
    fn part(&self, i: usize) -> String {
        let field = self.members.and_then(|m| m.fields.get(i)).map(|field| field.name);
        field.map_or_else(|| i.to_string(), |name| name.to_string())
    }

    fn variant(&self, i: usize) -> Option<Symbol> {
        self.members.and_then(|m| m.variants.get(i)).map(|v| v.name)
    }
}

/// A size and an alignment: `size 8, align 4`.
impl fmt::Display for Layout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size {}, align {}", self.size, self.align)
    }
}

/// `at 4`, or `at bit 3` in a bit-packed struct.
impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes(n) => write!(f, "at {n}"),
            Self::Bits(n) => write!(f, "at bit {n}"),
        }
    }
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let layout = &self.shape.layout;
        write!(f, "{layout}")?;
        match &self.shape.parts {
            Parts::Whole => {}
            Parts::Sequence(offsets) | Parts::Bits(offsets) => {
                for i in 0..offsets.len() {
                    let offset = self.shape.offset(i).expect("each part has its offset");
                    write!(f, "\n  {} {offset}", self.part(i))?;
                }
            }
            Parts::Array { stride, len } => write!(f, "\n  {len} items {stride} bytes apart")?,
            Parts::Enum { tag, payloads } => {
                match *tag {
                    Tag::Alone => {}
                    Tag::Index { bytes } => {
                        write!(f, "\n  tag in the {}-bit integer at 0", Self::bits(bytes))?;
                    }
                    Tag::Niche { empty, offset, bytes, value } => {
                        let name = self.variant(empty).map_or_else(|| empty.to_string(), |n| n.to_string());
                        write!(
                            f,
                            "\n  .{name} is {value} in the {}-bit integer at {offset}",
                            Self::bits(bytes)
                        )?;
                    }
                }
                for (i, offsets) in payloads.iter().enumerate() {
                    let name = self.variant(i).map_or_else(|| i.to_string(), |n| n.to_string());
                    write!(f, "\n  .{name}")?;
                    Self::offsets(f, offsets)?;
                }
            }
        }
        if let Some(niche) = layout.niche {
            write!(f, "\n  {niche}")?;
        }
        Ok(())
    }
}
