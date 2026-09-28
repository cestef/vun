//! Target descriptions read from their values (spec 13, section 7): a `lang.target.Spec` or
//! `lang.target.Host` evaluated at compile time becomes the typed description the rest of the
//! toolchain reads, member by member through the handles of its declaration.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_items::Item;
use vun_items::lang::target_Convention as convention;
use vun_items::lang::{
    target_Atomics as atomics, target_Attributes as attributes, target_CCompiler as compiler,
    target_FrameSizes as frames,
};
use vun_items::lang::{target_Backends as backends, target_Host as host, target_Spec as spec};
use vun_items::lang::{target_C as c, target_CInt as cint, target_Endian as endian};
use vun_target::{
    Atomics, Attributes, Backends, CCompiler, CInt, CTypes, Convention, Endian, FrameSizes, Host, TargetSpec,
};

use super::members::Members;
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

/// A description read from its value.
pub trait Read: Sized {
    /// # Errors
    /// Returns what in the value is not a literal of its type.
    fn read(value: &Value) -> Result<Self, Unread>;
}

/// An enum read by its variant: each declares, once, the variant of its `lang` enum each of its
/// values is.
pub trait Variants: Copy + PartialEq + 'static {
    const VARIANTS: &'static [(Item, Self)];

    /// The variant this value is, by its handle.
    ///
    /// # Panics
    /// Never: the table lists every value.
    fn item(self) -> Item {
        Self::VARIANTS
            .iter()
            .find(|&&(_, v)| v == self)
            .map(|&(item, _)| item)
            .expect("every value is listed")
    }
}

impl<T: Variants> Read for T {
    /// A variant, or its bare `.name` where no type was expected, as an attribute's argument is.
    fn read(value: &Value) -> Result<Self, Unread> {
        let (Value::Variant { name, .. } | Value::Name(name)) = value else { return Err(Unread::Record) };
        T::VARIANTS
            .iter()
            .find(|(variant, _)| *name == variant.symbol())
            .map(|&(_, v)| v)
            .ok_or(Unread::Record)
    }
}

impl Variants for Endian {
    const VARIANTS: &'static [(Item, Self)] = &[(endian::little, Self::Little), (endian::big, Self::Big)];
}

impl Variants for Convention {
    const VARIANTS: &'static [(Item, Self)] = &[
        (convention::sysv, Self::Sysv),
        (convention::aapcs64, Self::Aapcs64),
        (convention::darwin_arm64, Self::DarwinArm64),
        (convention::lp64d, Self::Lp64d),
        (convention::aapcs_vfp, Self::AapcsVfp),
        (convention::aapcs, Self::Aapcs),
        (convention::ilp32, Self::Ilp32),
        (convention::wasm32, Self::Wasm32),
    ];
}

impl Variants for Atomics {
    const VARIANTS: &'static [(Item, Self)] = &[(atomics::none, Self::None), (atomics::gnu, Self::Gnu)];
}

impl Variants for Attributes {
    const VARIANTS: &'static [(Item, Self)] = &[(attributes::none, Self::None), (attributes::gnu, Self::Gnu)];
}

impl Variants for FrameSizes {
    const VARIANTS: &'static [(Item, Self)] =
        &[(frames::none, Self::None), (frames::stack_usage, Self::StackUsage)];
}

impl Read for CInt {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { bits: m.int(cint::bits)?, align: m.int(cint::align)? })
    }
}

impl Read for CCompiler {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            command: m.string(compiler::command)?,
            archiver: m.string(compiler::archiver)?,
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
            isa: m.name(spec::isa)?,
            cpu: m.name(spec::cpu)?,
            os: m.name(spec::os)?,
            abi: m.name(spec::abi)?,
            convention: m.read(spec::convention)?,
            pointer_bits: m.width(spec::pointer_bits)?,
            register_bits: m.width(spec::register_bits)?,
            floats: m.floats(spec::floats)?,
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
