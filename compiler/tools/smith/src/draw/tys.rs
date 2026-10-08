//! Types drawn at random.

use super::Draw;
use crate::int::{BigInt, Range, Width};
use crate::ir::TypeId;
use crate::ty::{Nominal, Ty};
use vun_index::Idx;

/// The kinds of integer types a draw makes.
#[derive(Clone, Copy, vun_variants::Variants)]
enum Shape {
    /// One of `u8` to `i128`.
    Named,
    /// `uN` or `iN` of another width, stored in the next word up.
    Odd,
    /// Bounds of its own, small.
    Small,
    /// Bounds of its own, anywhere in a word.
    Spread,
}

impl Shape {
    const fn weight(self) -> usize {
        match self {
            Self::Named => 10,
            Self::Odd => 3,
            Self::Small => 3,
            Self::Spread => 2,
        }
    }
}

/// The kinds of types a draw makes.
#[derive(Clone, Copy, vun_variants::Variants)]
enum Kind {
    Int,
    Bool,
    Float,
    Tuple,
    Array,
    Struct,
    Enum,
    Option,
    Result,
}

impl Kind {
    const fn weight(self, depth: usize) -> usize {
        match self {
            Self::Int => 12,
            Self::Bool => 2,
            Self::Float => 3,
            _ if depth == 0 => 0,
            Self::Tuple => 2,
            Self::Array => 2,
            Self::Struct => 2,
            Self::Enum => 2,
            Self::Option => 2,
            Self::Result => 1,
        }
    }
}

impl Draw<'_> {
    /// An integer type.
    pub(super) fn range(&mut self) -> Range {
        let weights: Vec<usize> = Shape::ALL.iter().map(|s| s.weight()).collect();
        match Shape::ALL[self.entropy.weighted(&weights)] {
            Shape::Named => {
                let bits = *self.entropy.pick(&Width::NAMED);
                Width { signed: self.entropy.chance(1, 2), bits }.range()
            }
            Shape::Odd => {
                let bits = u32::try_from(self.entropy.between(1, 127)).expect("below 128");
                if self.entropy.chance(1, 2) && bits > 1 {
                    Range::signed(bits)
                } else {
                    Range::unsigned(bits)
                }
            }
            Shape::Small => {
                let lo = i64::try_from(self.entropy.between(0, 40)).expect("small") - 20;
                let len = i64::try_from(self.entropy.between(0, 60)).expect("small");
                Range::new(lo, lo + len)
            }
            Shape::Spread => {
                let bits = self.entropy.between(1, 70);
                let sign = BigInt::from(if self.entropy.chance(1, 2) { -1_i32 } else { 1 });
                let lo = BigInt::from(self.entropy.bits() >> (64 - bits.min(64))) * sign;
                let len = BigInt::from(self.entropy.bits() >> self.entropy.between(0, 63));
                Range::new(lo.clone(), lo + len)
            }
        }
    }

    /// A type to bind or to pass, no deeper than `depth` aggregates.
    pub(super) fn ty(&mut self, depth: usize) -> Ty {
        let declared: Vec<&Ty> =
            self.program.types.iter().map(|(_, t)| t).filter(|t| t.depth() <= depth).collect();
        if !declared.is_empty() && self.entropy.chance(1, 4) {
            return (*self.entropy.pick(&declared)).clone();
        }
        let weights: Vec<usize> = Kind::ALL.iter().map(|k| k.weight(depth)).collect();
        match Kind::ALL[self.entropy.weighted(&weights)] {
            Kind::Int => Ty::Int(self.range()),
            Kind::Bool => Ty::Bool,
            Kind::Float => Ty::Float(*self.entropy.pick(crate::float::Format::ALL)),
            Kind::Tuple => {
                let parts = self.entropy.between(2, self.config.parts.max(2));
                Ty::Tuple((0..parts).map(|_| self.ty(depth - 1)).collect())
            }
            Kind::Array => {
                let len = self.entropy.between(1, self.config.parts.max(1));
                Ty::Array(len, Box::new(self.ty(depth - 1)))
            }
            Kind::Struct => {
                let fields: Vec<Ty> = (0..self.entropy.between(1, self.config.parts.max(1)))
                    .map(|_| self.ty(depth - 1))
                    .collect();
                let id = TypeId::new(self.program.types.len());
                self.program.types.push(Ty::Struct(id, fields.clone()));
                Ty::Struct(id, fields)
            }
            Kind::Enum => {
                let variants: Vec<Vec<Ty>> = (0..self.entropy.between(1, self.config.parts.max(1)))
                    .map(|_| {
                        let held = self.entropy.between(0, 2);
                        (0..held).map(|_| self.ty(depth - 1)).collect()
                    })
                    .collect();
                let id = TypeId::new(self.program.types.len());
                self.program.types.push(Ty::Enum(Nominal::Declared(id), variants.clone()));
                Ty::Enum(Nominal::Declared(id), variants)
            }
            Kind::Option => Ty::option(self.ty(depth - 1)),
            Kind::Result => {
                let (ok, err) = (self.ty(depth - 1), self.ty(depth - 1));
                Ty::Enum(Nominal::Result, vec![vec![ok], vec![err]])
            }
        }
    }

    /// A type that holds everything `ty` does, possibly more: what a binding is annotated with.
    pub(super) fn wider(&mut self, ty: &Ty) -> Ty {
        match ty {
            Ty::Int(range) if self.entropy.chance(2, 3) => {
                let widths =
                    Width::NAMED.map(|bits| [false, true].map(|signed| Width { signed, bits }.range()));
                let holding: Vec<Range> = widths.into_iter().flatten().filter(|w| range.within(w)).collect();
                if holding.is_empty() { ty.clone() } else { Ty::Int(self.entropy.pick(&holding).clone()) }
            }
            Ty::Tuple(parts) => Ty::Tuple(parts.iter().map(|p| self.wider(p)).collect()),
            Ty::Array(len, item) => Ty::Array(*len, Box::new(self.wider(item))),
            _ => ty.clone(),
        }
    }
}
