//! Compile-time values at run time: each encoded as the bytes a value of its type holds, integers
//! in their storage and the target's byte order, aggregates and variants by their shapes, and
//! addresses of functions, link symbols and borrowed constants as relocations.

use vun_lir::{Access, Address, Reloc, Scalar, Value};
use vun_origin::OriginId;
use vun_sema::Primitive;
use vun_sema::eval::Instantiation;
use vun_sema::int::Integer;
use vun_sema::layout::{Parts, Tag};
use vun_sema::member::Members;
use vun_sema::primitives::mem::Mem;
use vun_sema::ty::{Ty, TyKind, Value as Constant};
use vun_target::Endian;

use super::FnLower;
use crate::program::Lowering;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered};

/// A value's bytes, and the addresses to write into them.
pub(crate) struct Bytes {
    pub(crate) bytes: Vec<u8>,
    pub(crate) relocs: Vec<Reloc>,
}

impl Lowering<'_, '_> {
    /// `value` as a value of type `ty` holds it.
    pub(crate) fn encode(&mut self, value: &Constant, ty: Ty, at: OriginId) -> Result<Bytes, Unlowered> {
        let size = self.types.shape(ty, at)?.layout.size;
        let mut out = Bytes { bytes: vec![0; usize::try_from(size).unwrap_or_default()], relocs: Vec::new() };
        self.put(value, ty, &mut out, 0, at)?;
        Ok(out)
    }

    /// Writes the integer `n` into `bytes` in `width` bytes at `at`, in the target's byte order.
    fn int(&self, n: &Integer, width: u64, bytes: &mut [u8], at: u64) {
        let modulus = Integer::power_of_two(u32::try_from(width * u64::from(u8::BITS)).unwrap_or(u128::BITS));
        let bits = n.modulo(&modulus).and_then(|m| m.to_u128()).unwrap_or_default().to_le_bytes();
        let (at, width) =
            (usize::try_from(at).unwrap_or_default(), usize::try_from(width).unwrap_or_default());
        let slot = &mut bytes[at..at + width];
        slot.copy_from_slice(&bits[..width]);
        if self.types.endian == Endian::Big {
            slot.reverse();
        }
    }

    fn put(
        &mut self,
        value: &Constant,
        ty: Ty,
        out: &mut Bytes,
        base: u64,
        at: OriginId,
    ) -> Result<(), Unlowered> {
        let unsupported = || Unlowered::unsupported(at, Form::Constant);
        let cx = self.types.cx;
        if let (Constant::Int(n), Some((int, _))) = (value, self.types.int(ty)) {
            self.int(n, int.bytes(0), &mut out.bytes, base);
            return Ok(());
        }
        // A cell holds its value at compile time (`lang.mem.cell`).
        if let TyKind::Primitive(Primitive::Mem(Mem::Cell), args) = &*ty {
            let inner = args.first().and_then(Constant::as_type).ok_or_else(unsupported)?;
            return self.put(value, inner, out, base, at);
        }
        let shape = self.types.shape(ty, at)?;
        let address = |target| Reloc { offset: base, target, addend: 0 };
        match value {
            Constant::Fn(f) | Constant::Closure { f, .. } if matches!(&*ty, TyKind::Primitive(..)) => {
                let id = self.function(&Instantiation::new(f.clone(), Vec::new()), at)?;
                out.relocs.push(address(Address::Func(id)));
            }
            Constant::Linked(decl) => {
                let target = self.linked(decl, at)?;
                out.relocs.push(address(target));
            }
            Constant::Struct { fields, .. } => {
                let members = cx.get::<Members>(&ty).ok_or_else(unsupported)?;
                let Parts::Sequence(offsets) = &shape.parts else { return Err(unsupported()) };
                for (name, v) in fields {
                    let index =
                        members.fields.iter().position(|f| f.name == *name).ok_or_else(unsupported)?;
                    let field = members.fields[index].ty.as_ref().map_err(|_| unsupported())?;
                    self.put(v, *field, out, base + offsets[index], at)?;
                }
            }
            Constant::Tuple(items) | Constant::Closure { captures: items, .. } => {
                let parts = self.types.parts(ty).ok_or_else(unsupported)?;
                let Parts::Sequence(offsets) = &shape.parts else { return Err(unsupported()) };
                for ((v, part), offset) in items.iter().zip(parts).zip(offsets) {
                    self.put(v, part, out, base + offset, at)?;
                }
            }
            Constant::Array(items)
                if let (Some((_, item)), Parts::Array { stride, .. }) = (ty.as_array(), &shape.parts) =>
            {
                for (i, v) in (0..).zip(items) {
                    self.put(v, item, out, base + i * stride, at)?;
                }
            }
            Constant::Variant { name, payload, .. } => {
                let members = cx.get::<Members>(&ty).ok_or_else(unsupported)?;
                let index = members.variants.iter().position(|v| v.name == *name).ok_or_else(unsupported)?;
                let Parts::Enum { tag, payloads } = &shape.parts else { return Err(unsupported()) };
                match *tag {
                    Tag::Index { bytes } => self.int(&Integer::from(index), bytes, &mut out.bytes, base),
                    Tag::Niche { empty, offset, bytes, value } if empty == index => {
                        self.int(&Integer::from(value), bytes, &mut out.bytes, base + offset);
                    }
                    Tag::Alone | Tag::Niche { .. } => {}
                }
                let items = members.variants[index].payload.iter();
                for ((v, item), offset) in payload.iter().zip(items).zip(&payloads[index]) {
                    self.put(v, *item.as_ref().map_err(|_| unsupported())?, out, base + offset, at)?;
                }
            }
            // A shared borrow of a compile-time value holds the value: it lives in a data object,
            // with its length for a slice.
            _ if let Some(pointee) = ty.pointee() => {
                let (stored, len) = match (pointee.slice_items(), value) {
                    (Some(item), Constant::Array(items)) => {
                        (TyKind::array(Integer::from(items.len()), item), Some(items.len()))
                    }
                    _ => (pointee, None),
                };
                let bytes = self.encode(value, stored, at)?;
                let align = self.types.shape(stored, at)?.layout.align;
                let data = self.constant(bytes.bytes, bytes.relocs, align, at);
                out.relocs.push(address(Address::Data(data)));
                if let Some(len) = len {
                    let word = self.types.word.bytes(self.types.pointer_bits());
                    self.int(&Integer::from(len), word, &mut out.bytes, base + word);
                }
            }
            _ => return Err(unsupported()),
        }
        Ok(())
    }
}

impl FnLower<'_, '_, '_> {
    /// Writes the constant `value` as a value of type `ty` at `dest`.
    pub(crate) fn write_const(&mut self, value: &Constant, dest: Value, ty: Ty) -> Result<(), Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            Repr::Zero => Ok(()),
            Repr::Scalar(scalar) => {
                let value = self.scalar_const(value, ty, scalar)?;
                self.b.store(value, dest, Access::Plain, self.at);
                Ok(())
            }
            Repr::Memory(layout) => {
                let bytes = self.l.encode(value, ty, self.at)?;
                let data = self.l.constant(bytes.bytes, bytes.relocs, layout.align, self.at);
                let from = self.b.addr(Address::Data(data), self.at);
                self.b.copy(dest, from, layout.size, self.at);
                Ok(())
            }
        }
    }

    /// The constant `value` of type `ty`, held as `scalar`.
    pub(crate) fn scalar_const(
        &mut self,
        value: &Constant,
        ty: Ty,
        scalar: Scalar,
    ) -> Result<Value, Unlowered> {
        let bytes = self.l.encode(value, ty, self.at)?;
        if let [reloc] = bytes.relocs.as_slice() {
            let addr = self.b.addr(reloc.target, self.at);
            return Ok(self.at_offset(addr, u64::try_from(reloc.addend).unwrap_or_default()));
        }
        let mut raw = [0u8; 16];
        raw[..bytes.bytes.len()].copy_from_slice(&bytes.bytes);
        let mut bits = u128::from_le_bytes(raw);
        if self.l.types.endian == Endian::Big {
            bits = u128::from_be_bytes(raw) >> (128 - 8 * bytes.bytes.len());
        }
        Ok(self.b.constant(scalar, bits, self.at))
    }
}
