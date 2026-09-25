//! The file's integers, declared from the target's C types (`lang.target.C`): `uN` and `iN` for
//! each width a C integer type has, and checks of everything the backend relies on of them, made
//! by the C compiler: that `char` is 8 bits, and the size and alignment of each type and of an
//! address. Nothing about C's types is assumed.

use vun_lir::Scalar;
use vun_origin::OriginId;
use vun_target::CInt;

use super::Emitter;
use crate::ast::{Aggregate, Base, Binary, Decl, Expr, Ident, Item, Own, Suffix, Ty};
use vun_backend::{Lack, Missing};

/// The widths of the integers LIR has that C can declare.
const WIDTHS: [u32; 4] = [8, 16, 32, 64];

impl Emitter<'_> {
    /// The target's C integer types, with their width and alignment; `char` is a byte.
    fn bases(&self) -> [(Base, CInt); 5] {
        let c = &self.target.c;
        let byte = CInt { bits: u8::BITS, align: 1 };
        [
            (Base::Char, byte),
            (Base::Short, c.short),
            (Base::Int, c.int),
            (Base::Long, c.long),
            (Base::LongLong, c.long_long),
        ]
    }

    /// The first C integer type of the target with `bits` bits.
    fn base(&self, bits: u32) -> Option<(Base, CInt)> {
        self.bases().into_iter().find(|(_, c)| c.bits == bits)
    }

    /// The declarations of `uN` and `iN`, and their checks.
    pub(super) fn ints(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        let char_max = Expr::Small(-1).cast(Ty::Base { base: Base::Char, signed: false });
        self.check(Expr::binary(Binary::Eq, char_max, Expr::small(u64::from(u8::MAX))));
        for bits in WIDTHS {
            let Some((base, c)) = self.base(bits) else { continue };
            for signed in [false, true] {
                items.push(Item::Typedef(Decl::new(
                    Ty::Base { base, signed },
                    Ident::Own(Own::Int { bits, signed }),
                )));
            }
            let bytes = u64::from(bits / u8::BITS);
            self.check_size(Ty::uint(bits), bytes);
            items.push(self.aligned_by(Own::Aligned(bits), Ty::uint(bits), u64::from(c.align) + bytes));
        }
        let address = u64::from(self.unit.pointer_bits / u8::BITS);
        let pointer =
            self.aligned_by(Own::Aligned(0), Ty::address(), u64::from(self.target.c.pointer_align) + address);
        items.push(pointer);
        items
    }

    /// A `struct` of a byte then a `ty`, whose size is checked to be `size`: the alignment of
    /// `ty` plus its size, when C aligns it as the target says.
    fn aligned_by(&mut self, own: Own, ty: Ty, size: u64) -> Item {
        let tag = Ident::Own(own);
        self.check_size(Ty::Struct(tag), size);
        let fields = vec![Decl::new(Ty::BYTE, Ident::field(0)), Decl::new(ty, Ident::field(1))];
        Item::Aggregate { kind: Aggregate::Struct, tag, fields }
    }

    /// `uN` or `iN`, if some C integer type of the target has `bits` bits.
    pub(super) fn int(&self, bits: u32, signed: bool, at: OriginId) -> Result<Ty, Lack> {
        self.base(bits).map(|_| Ty::Int { bits, signed }).ok_or(Missing::Width(bits).at(at))
    }

    /// The integer as wide as an address, which [`Emitter::file`] checks exists.
    pub(super) fn word(&self) -> Ty {
        Ty::uint(self.unit.pointer_bits)
    }

    /// The type holding a value of `s`.
    pub(super) fn scalar(&self, s: Scalar, at: OriginId) -> Result<Ty, Lack> {
        match s {
            Scalar::I128 => Err(Missing::Int128.at(at)),
            Scalar::F16 => Err(Missing::Half.at(at)),
            Scalar::F32 => Ok(Ty::Float),
            Scalar::F64 => Ok(Ty::Double),
            Scalar::Ptr => Ok(Ty::address()),
            Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::I64 => self.int(s.bits(0), false, at),
        }
    }

    /// The width of `int`: operands narrower than it are converted to `unsigned` first.
    pub(super) fn int_bits(&self) -> u32 {
        self.target.c.int.bits
    }

    /// The unsigned integer aligned to `align` bytes, which slots and data objects are arrays and
    /// unions of, and its size in bytes.
    pub(super) fn aligned(&self, align: u64, at: OriginId) -> Result<(Ty, u64), Lack> {
        let found =
            self.bases().into_iter().find(|(_, c)| u64::from(c.align) == align && WIDTHS.contains(&c.bits));
        let (_, c) = found.ok_or(Missing::Align(align).at(at))?;
        Ok((Ty::uint(c.bits), u64::from(c.bits / u8::BITS)))
    }

    /// The unsigned integer aligned the most, which a slot aligned past every C type lies in an
    /// array of, and its size in bytes.
    pub(super) fn most_aligned(&self) -> (Ty, u64) {
        let widths = self.bases().into_iter().filter(|(_, c)| WIDTHS.contains(&c.bits));
        let c = widths.map(|(_, c)| c).max_by_key(|c| c.align).expect("`char` is one");
        (Ty::uint(c.bits), u64::from(c.bits / u8::BITS))
    }

    /// The integer constant `value` of `bits` bits, of type `uN`.
    pub(super) fn literal(&self, value: u128, bits: u32, at: OriginId) -> Result<Expr, Lack> {
        let (base, _) = self.base(bits).ok_or(Missing::Width(bits).at(at))?;
        let (suffix, exact) = match base {
            Base::Char | Base::Short => (Suffix::Unsigned, false),
            Base::Int => (Suffix::Unsigned, true),
            Base::Long => (Suffix::Long, true),
            Base::LongLong => (Suffix::LongLong, true),
        };
        let literal = Expr::Literal { value, suffix };
        Ok(if exact { literal } else { literal.cast(Ty::uint(bits)) })
    }

    /// A constant of `s` with these bits; a float that is not finite is made from its bits.
    pub(super) fn constant(&mut self, s: Scalar, bits: u128, at: OriginId) -> Result<Expr, Lack> {
        Ok(match s {
            Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::I64 => self.literal(bits, s.bits(0), at)?,
            Scalar::F32 | Scalar::F64 => {
                let width = s.bits(0);
                let value = u64::try_from(bits).expect("a float's bits fit its width");
                let finite = if width == 32 {
                    f32::from_bits(u32::try_from(value).expect("32 bits")).is_finite()
                } else {
                    f64::from_bits(value).is_finite()
                };
                if finite {
                    Expr::Float { bits: width, value }
                } else {
                    self.helpers.insert(Own::FloatOf(width));
                    let helper = Expr::Name(Ident::Own(Own::FloatOf(width)));
                    Expr::Call(Box::new(helper), vec![self.literal(bits, width, at)?])
                }
            }
            Scalar::Ptr => {
                self.literal(bits, self.unit.pointer_bits, at)?.cast(self.word()).cast(Ty::address())
            }
            Scalar::I128 | Scalar::F16 => return Err(self.scalar(s, at).expect_err("refused by `scalar`")),
        })
    }
}
