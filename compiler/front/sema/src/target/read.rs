//! Target descriptions read from their values (spec 13, section 7): a `lang.target.Spec` or
//! `lang.target.Host` evaluated at compile time becomes the typed description the rest of the
//! toolchain reads, member by member through the handles of its declaration.

use std::fmt;

use vun_intern::Symbol;
use vun_items::Item;
use vun_items::lang::{Option as option, target_C as c, target_CInt as cint, target_Endian as endian};
use vun_items::lang::{target_Atomics as atomics, target_CCompiler as compiler};
use vun_items::lang::{target_Host as host, target_Spec as spec};
use vun_target::{Atomics, CCompiler, CInt, CTypes, Endian, Host, TargetSpec};

use crate::ty::Value;

/// Why a value is not the description its type promises: it is not evaluated yet, or not a
/// literal of its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unread {
    /// The member lacks, or holds another shape than its type gives.
    Member(Item),
    /// The value is not a `struct` literal.
    Record,
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Member(item) => {
                write!(f, "the member `{}` is not a literal of its type", item.path().join("."))
            }
            Self::Record => f.write_str("the value is not a `struct` literal"),
        }
    }
}

impl std::error::Error for Unread {}

/// A description read from its value.
pub trait Read: Sized {
    /// # Errors
    /// Returns what in the value is not a literal of its type.
    fn read(value: &Value) -> Result<Self, Unread>;
}

/// The members of a `struct` value.
struct Members<'v>(&'v [(Symbol, Value)]);

impl<'v> Members<'v> {
    fn of(value: &'v Value) -> Result<Self, Unread> {
        match value {
            Value::Struct { fields, .. } => Ok(Self(fields)),
            _ => Err(Unread::Record),
        }
    }

    fn get(&self, member: Item) -> Result<&'v Value, Unread> {
        let symbol = member.symbol();
        self.0.iter().find(|(name, _)| *name == symbol).map(|(_, v)| v).ok_or(Unread::Member(member))
    }

    fn name(&self, member: Item) -> Result<Symbol, Unread> {
        match self.get(member)? {
            Value::Name(name) => Ok(*name),
            _ => Err(Unread::Member(member)),
        }
    }

    fn int(&self, member: Item) -> Result<u32, Unread> {
        match self.get(member)? {
            Value::Int(n) => n.to_u128().and_then(|n| u32::try_from(n).ok()).ok_or(Unread::Member(member)),
            _ => Err(Unread::Member(member)),
        }
    }

    fn read<T: Read>(&self, member: Item) -> Result<T, Unread> {
        T::read(self.get(member)?).map_err(|_| Unread::Member(member))
    }

    /// A `?&[u8]` naming a symbol.
    fn symbol(&self, member: Item) -> Result<Option<Symbol>, Unread> {
        match self.get(member)? {
            Value::Variant { name, .. } if *name == option::none.symbol() => Ok(None),
            Value::Variant { name, payload, .. } if *name == option::some.symbol() => {
                match payload.as_slice() {
                    [text] => Self::text(text).map(Some).ok_or(Unread::Member(member)),
                    _ => Err(Unread::Member(member)),
                }
            }
            _ => Err(Unread::Member(member)),
        }
    }

    /// A `&[u8]` as the text it holds.
    fn string(&self, member: Item) -> Result<Symbol, Unread> {
        Self::text(self.get(member)?).ok_or(Unread::Member(member))
    }

    /// A `&[&[u8]]` as the texts it holds.
    fn strings(&self, member: Item) -> Result<Vec<Symbol>, Unread> {
        match self.get(member)? {
            Value::Array(items) | Value::Tuple(items) => {
                items.iter().map(|t| Self::text(t).ok_or(Unread::Member(member))).collect()
            }
            _ => Err(Unread::Member(member)),
        }
    }

    /// The UTF-8 text a byte array holds.
    fn text(value: &Value) -> Option<Symbol> {
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

impl Read for Endian {
    fn read(value: &Value) -> Result<Self, Unread> {
        match value {
            Value::Variant { name, .. } if *name == endian::little.symbol() => Ok(Self::Little),
            Value::Variant { name, .. } if *name == endian::big.symbol() => Ok(Self::Big),
            _ => Err(Unread::Record),
        }
    }
}

impl Read for CInt {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { bits: m.int(cint::bits)?, align: m.int(cint::align)? })
    }
}

impl Read for Atomics {
    fn read(value: &Value) -> Result<Self, Unread> {
        match value {
            Value::Variant { name, .. } if *name == atomics::none.symbol() => Ok(Self::None),
            Value::Variant { name, .. } if *name == atomics::gnu.symbol() => Ok(Self::Gnu),
            _ => Err(Unread::Record),
        }
    }
}

impl Read for CCompiler {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            command: m.string(compiler::command)?,
            flags: m.strings(compiler::flags)?,
            atomics: m.read(compiler::atomics)?,
        })
    }
}

impl Read for CTypes {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            short: m.read(c::short)?,
            int: m.read(c::int)?,
            long: m.read(c::long)?,
            long_long: m.read(c::long_long)?,
            pointer_align: m.int(c::pointer_align)?,
            compiler: m.read(c::compiler)?,
        })
    }
}

impl Read for TargetSpec {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            arch: m.name(spec::arch)?,
            cpu: m.name(spec::cpu)?,
            os: m.name(spec::os)?,
            abi: m.name(spec::abi)?,
            pointer_bits: m.int(spec::pointer_bits)?,
            endian: m.read(spec::endian)?,
            object_format: m.name(spec::object_format)?,
            entry_symbol: m.symbol(spec::entry_symbol)?,
            default_backend: m.name(spec::default_backend)?,
            default_linker: m.name(spec::default_linker)?,
            c: m.read(spec::c)?,
        })
    }
}

impl Read for Host {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { arch: m.name(host::arch)?, os: m.name(host::os)?, target: m.read(host::target)? })
    }
}
