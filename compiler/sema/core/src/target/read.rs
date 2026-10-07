//! Target descriptions read from their values (spec 13, section 7): a `lang.target.Spec` or
//! `lang.target.Host` evaluated at compile time becomes the typed description the rest of the
//! toolchain reads, member by member through the handles of its declaration.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_items::Item;
use vun_items::lang::Option as option;
use vun_items::lang::target_Convention as convention;
use vun_items::lang::target_Threads as threads;
use vun_items::lang::target_Version as version;
use vun_items::lang::{
    target_Atomics as atomics, target_Attributes as attributes, target_BuildsFor as builds_for,
    target_CCompiler as compiler, target_FrameSizes as frames, target_TailCalls as tail_calls,
};
use vun_items::lang::{target_Backends as backends, target_Spec as spec};
use vun_items::lang::{target_C as c, target_CInt as cint, target_Endian as endian};
use vun_items::std::build_Output as output;
use vun_sema_ty::Value;
use vun_target::{
    Atomics, Attributes, Backends, BuildsFor, CCompiler, CInt, CTypes, Convention, Endian, FrameSizes,
    Output, TailCalls, TargetSpec, Threads, Version,
};

use super::members::Members;

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

impl<T: Read> Read for Option<T> {
    /// `.none`, or `.some(value)` with the value read.
    fn read(value: &Value) -> Result<Self, Unread> {
        match value {
            Value::Variant { name, .. } if *name == option::none.symbol() => Ok(None),
            Value::Variant { name, payload, .. } if *name == option::some.symbol() => {
                match payload.as_slice() {
                    [value] => T::read(value).map(Some),
                    _ => Err(Unread::Record),
                }
            }
            _ => Err(Unread::Record),
        }
    }
}

impl Variants for Threads {
    const VARIANTS: &'static [(Item, Self)] = &[(threads::one, Self::One), (threads::many, Self::Many)];
}

impl Variants for Endian {
    const VARIANTS: &'static [(Item, Self)] = &[(endian::little, Self::Little), (endian::big, Self::Big)];
}

impl Variants for Convention {
    const VARIANTS: &'static [(Item, Self)] = &[
        (convention::sysv, Self::Sysv),
        (convention::aapcs64, Self::Aapcs64),
        (convention::darwin_arm64, Self::DarwinArm64),
        (convention::win64, Self::Win64),
        (convention::lp64d, Self::Lp64d),
        (convention::aapcs_vfp, Self::AapcsVfp),
        (convention::aapcs, Self::Aapcs),
        (convention::ilp32, Self::Ilp32),
        (convention::wasm32, Self::Wasm32),
        (convention::cdecl, Self::Cdecl),
        (convention::stdcall, Self::Stdcall),
        (convention::fastcall, Self::Fastcall),
        (convention::thiscall, Self::Thiscall),
    ];
}

impl Variants for BuildsFor {
    const VARIANTS: &'static [(Item, Self)] =
        &[(builds_for::target, Self::Target), (builds_for::host, Self::Host)];
}

impl Variants for Atomics {
    const VARIANTS: &'static [(Item, Self)] = &[(atomics::none, Self::None), (atomics::gnu, Self::Gnu)];
}

impl Variants for Attributes {
    const VARIANTS: &'static [(Item, Self)] = &[(attributes::none, Self::None), (attributes::gnu, Self::Gnu)];
}

impl Variants for TailCalls {
    const VARIANTS: &'static [(Item, Self)] =
        &[(tail_calls::none, Self::None), (tail_calls::musttail, Self::Musttail)];
}

impl Variants for FrameSizes {
    const VARIANTS: &'static [(Item, Self)] =
        &[(frames::none, Self::None), (frames::stack_usage, Self::StackUsage)];
}

impl Variants for Output {
    const VARIANTS: &'static [(Item, Self)] = &[
        (output::executable, Self::Executable),
        (output::shared, Self::Shared),
        (output::r#static, Self::Static),
    ];
}

impl Read for CInt {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { bits: m.int(cint::bits)?, align: m.int(cint::align)? })
    }
}

impl Read for Version {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { major: m.int(version::major)?, minor: m.int(version::minor)? })
    }
}

impl Read for CCompiler {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self {
            command: m.string(compiler::command)?,
            builds_for: m.read(compiler::builds_for)?,
            archiver: m.string(compiler::archiver)?,
            flags: m.strings(compiler::flags)?,
            optimize: m.strings(compiler::optimize)?,
            shared: m.strings(compiler::shared)?,
            link_flags: m.strings(compiler::link_flags)?,
            libraries: m.strings(compiler::libraries)?,
            atomics: m.read(compiler::atomics)?,
            attributes: m.read(compiler::attributes)?,
            tail_calls: m.read(compiler::tail_calls)?,
            frame_sizes: m.read(compiler::frame_sizes)?,
            version_flag: m.symbol(compiler::version_flag)?,
            sysroot: m.symbol(compiler::sysroot)?,
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
            features: m.names(spec::features)?,
            cpu: m.name(spec::cpu)?,
            os: m.name(spec::os)?,
            abi: m.name(spec::abi)?,
            convention: m.read(spec::convention)?,
            accepts: m.list(spec::accepts)?,
            pointer_bits: m.width(spec::pointer_bits)?,
            register_bits: m.width(spec::register_bits)?,
            floats: m.floats(spec::floats)?,
            threads: m.read(spec::threads)?,
            endian: m.read(spec::endian)?,
            object_format: m.name(spec::object_format)?,
            entry_symbol: m.symbol(spec::entry_symbol)?,
            os_version: m.option(spec::os_version)?.map(Version::read).transpose()?,
            backends: m.read(spec::backends)?,
            linker: m.name(spec::linker)?,
            c: m.read(spec::c)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_convention_is_named_as_the_variant_of_lang_target_it_reads_from() {
        for &(item, convention) in Convention::VARIANTS {
            assert_eq!(item.symbol().as_str(), convention.name());
        }
    }
}
