//! The members of a `struct` value a target description is read from, each by its declaration's
//! handle and as the shape its type gives.

use vun_intern::Symbol;
use vun_items::Item;
use vun_items::lang::Option as option;
use vun_target::Floats;

use super::read::{Read, Unread};
use crate::float::Format;
use crate::ty::Value;

/// The widths of an address or a register a target may have.
const WIDTHS: [u32; 3] = [16, 32, 64];

/// The members of a `struct` value.
pub(crate) struct Members<'v>(&'v [(Symbol, Value)]);

impl<'v> Members<'v> {
    pub(crate) fn of(value: &'v Value) -> Result<Self, Unread> {
        match value {
            Value::Struct { fields, .. } => Ok(Self(fields)),
            _ => Err(Unread::Record),
        }
    }

    pub(crate) fn get(&self, member: Item) -> Result<&'v Value, Unread> {
        let symbol = member.symbol();
        self.0.iter().find(|(name, _)| *name == symbol).map(|(_, v)| v).ok_or(Unread::Member(member))
    }

    pub(crate) fn name(&self, member: Item) -> Result<Symbol, Unread> {
        match self.get(member)? {
            Value::Name(name) => Ok(*name),
            _ => Err(Unread::Member(member)),
        }
    }

    pub(crate) fn int<N: TryFrom<u128>>(&self, member: Item) -> Result<N, Unread> {
        match self.get(member)? {
            Value::Int(n) => n.to_u128().and_then(|n| N::try_from(n).ok()).ok_or(Unread::Member(member)),
            _ => Err(Unread::Member(member)),
        }
    }

    pub(crate) fn read<T: Read>(&self, member: Item) -> Result<T, Unread> {
        T::read(self.get(member)?).map_err(|_| Unread::Member(member))
    }

    /// The width of an address, one of `ADDRESS_BITS`.
    pub(super) fn width(&self, member: Item) -> Result<u32, Unread> {
        let bits = self.int(member)?;
        WIDTHS.contains(&bits).then_some(bits).ok_or(Unread::Member(member))
    }

    /// A `&[type]` of float types.
    pub(super) fn floats(&self, member: Item) -> Result<Floats, Unread> {
        let unread = || Unread::Member(member);
        let items = self.get(member)?.clone().items().ok_or_else(unread)?;
        items.iter().try_fold(Floats::default(), |floats, item| {
            let format = item.as_type().and_then(|t| Format::of(&t)).ok_or_else(unread)?;
            floats.with(format.bits()).ok_or_else(unread)
        })
    }

    /// A `&[lang.meta.Name]`.
    pub(super) fn names(&self, member: Item) -> Result<Vec<Symbol>, Unread> {
        match self.get(member)? {
            Value::Array(items) | Value::Tuple(items) => items
                .iter()
                .map(|n| match n {
                    Value::Name(name) => Ok(*name),
                    _ => Err(Unread::Member(member)),
                })
                .collect(),
            _ => Err(Unread::Member(member)),
        }
    }

    /// The value a `?T` holds, if any.
    pub(crate) fn option(&self, member: Item) -> Result<Option<&'v Value>, Unread> {
        match self.get(member)? {
            Value::Variant { name, .. } if *name == option::none.symbol() => Ok(None),
            Value::Variant { name, payload, .. } if *name == option::some.symbol() => {
                match payload.as_slice() {
                    [value] => Ok(Some(value)),
                    _ => Err(Unread::Member(member)),
                }
            }
            _ => Err(Unread::Member(member)),
        }
    }

    /// A `?&[u8]` naming a symbol.
    pub(crate) fn symbol(&self, member: Item) -> Result<Option<Symbol>, Unread> {
        self.option(member)?.map(|text| Self::text(text).ok_or(Unread::Member(member))).transpose()
    }

    /// The variant a `?E` holds, if any.
    pub(crate) fn variant(&self, member: Item) -> Result<Option<Symbol>, Unread> {
        self.option(member)?
            .map(|value| match value {
                Value::Variant { name, .. } => Ok(*name),
                _ => Err(Unread::Member(member)),
            })
            .transpose()
    }

    /// The items of a `&[T]`, each read.
    pub(crate) fn list<T: Read>(&self, member: Item) -> Result<Vec<T>, Unread> {
        match self.get(member)? {
            Value::Array(items) | Value::Tuple(items) => {
                items.iter().map(|item| T::read(item).map_err(|_| Unread::Member(member))).collect()
            }
            _ => Err(Unread::Member(member)),
        }
    }

    /// A `&[u8]` as the text it holds.
    pub(crate) fn string(&self, member: Item) -> Result<Symbol, Unread> {
        Self::text(self.get(member)?).ok_or(Unread::Member(member))
    }

    /// A `&[&[u8]]` as the texts it holds.
    pub(super) fn strings(&self, member: Item) -> Result<Vec<Symbol>, Unread> {
        match self.get(member)? {
            Value::Array(items) | Value::Tuple(items) => {
                items.iter().map(|t| Self::text(t).ok_or(Unread::Member(member))).collect()
            }
            _ => Err(Unread::Member(member)),
        }
    }

    /// The UTF-8 text a byte array holds.
    pub(super) fn text(value: &Value) -> Option<Symbol> {
        let Value::Array(bytes) = value else { return None };
        let bytes = bytes
            .iter()
            .map(|b| match b {
                Value::Int(n) => n.to_u128().and_then(|n| u8::try_from(n).ok()),
                _ => None,
            })
            .collect::<Option<Vec<u8>>>()?;
        String::from_utf8(bytes).ok().map(|s| Symbol::intern(&s))
    }
}
