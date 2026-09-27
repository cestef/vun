//! Target descriptions read from their values (spec 13, section 7): a `lang.target.Spec` or
//! `lang.target.Host` evaluated at compile time becomes the typed description the rest of the
//! toolchain reads, member by member through the handles of its declaration.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_items::Item;
use vun_items::lang::target_Convention as convention;
use vun_items::lang::{Option as option, target_C as c, target_CInt as cint, target_Endian as endian};
use vun_items::lang::{
    target_Atomics as atomics, target_Attributes as attributes, target_CCompiler as compiler,
    target_FrameSizes as frames,
};
use vun_items::lang::{target_Backends as backends, target_Host as host, target_Spec as spec};
use vun_target::{
    Atomics, Attributes, Backends, CCompiler, CInt, CTypes, Convention, Endian, FrameSizes, Host, TargetSpec,
};

use crate::ty::Value;

/// Why a value is not the description its type promises: it is not evaluated yet, or not a
/// literal of its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

/// The widths of an address a target may have.
const ADDRESS_BITS: [u32; 3] = [16, 32, 64];

/// A description read from its value.
pub trait Read: Sized {
    /// # Errors
    /// Returns what in the value is not a literal of its type.
    fn read(value: &Value) -> Result<Self, Unread>;
}

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
    fn address_bits(&self, member: Item) -> Result<u32, Unread> {
        let bits = self.int(member)?;
        ADDRESS_BITS.contains(&bits).then_some(bits).ok_or(Unread::Member(member))
    }

    /// A `&[lang.meta.Name]`.
    fn names(&self, member: Item) -> Result<Vec<Symbol>, Unread> {
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

impl Read for Convention {
    fn read(value: &Value) -> Result<Self, Unread> {
        const VARIANTS: [(Item, Convention); 7] = [
            (convention::sysv, Convention::Sysv),
            (convention::aapcs64, Convention::Aapcs64),
            (convention::darwin_arm64, Convention::DarwinArm64),
            (convention::lp64d, Convention::Lp64d),
            (convention::aapcs_vfp, Convention::AapcsVfp),
            (convention::ilp32, Convention::Ilp32),
            (convention::wasm32, Convention::Wasm32),
        ];
        let Value::Variant { name, .. } = value else { return Err(Unread::Record) };
        VARIANTS.iter().find(|(variant, _)| *name == variant.symbol()).map(|&(_, c)| c).ok_or(Unread::Record)
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

impl Read for Attributes {
    fn read(value: &Value) -> Result<Self, Unread> {
        match value {
            Value::Variant { name, .. } if *name == attributes::none.symbol() => Ok(Self::None),
            Value::Variant { name, .. } if *name == attributes::gnu.symbol() => Ok(Self::Gnu),
            _ => Err(Unread::Record),
        }
    }
}

impl Read for FrameSizes {
    fn read(value: &Value) -> Result<Self, Unread> {
        match value {
            Value::Variant { name, .. } if *name == frames::none.symbol() => Ok(Self::None),
            Value::Variant { name, .. } if *name == frames::stack_usage.symbol() => Ok(Self::StackUsage),
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
            optimize: m.strings(compiler::optimize)?,
            link_flags: m.strings(compiler::link_flags)?,
            atomics: m.read(compiler::atomics)?,
            attributes: m.read(compiler::attributes)?,
            frame_sizes: m.read(compiler::frame_sizes)?,
        })
    }
}

impl Read for Backends {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { debug: m.names(backends::debug)?, release: m.names(backends::release)? })
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
            convention: m.read(spec::convention)?,
            pointer_bits: m.address_bits(spec::pointer_bits)?,
            endian: m.read(spec::endian)?,
            object_format: m.name(spec::object_format)?,
            entry_symbol: m.symbol(spec::entry_symbol)?,
            backends: m.read(spec::backends)?,
            linker: m.name(spec::linker)?,
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
