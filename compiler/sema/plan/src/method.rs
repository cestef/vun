//! How a program reaches its device, read from `std.build.Flash`.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_items::std::{
    build_Argument as argument, build_Delivery as delivery, build_Drive as drive, build_Flash as flash,
    build_Invocation as invocation, build_Probe as probe, build_Uf2 as uf2,
};
use vun_sema::target::{Members, Read, Unread};
use vun_sema_ty::Value;

/// How a program reaches its device (`std.build.Flash`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Method {
    /// Through a debug probe attached to `chip`.
    Probe { chip: Symbol },
    /// As a file of blocks for the chip family numbered `family`, which its boot ROM takes over USB
    /// mass storage, given to the device by the first of `deliver` that reaches one.
    Uf2 { family: u32, deliver: Vec<Delivery> },
    /// Through the device firmware upgrade protocol.
    Dfu,
}

/// A way to give a file to a device (`std.build.Delivery`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Delivery {
    /// Copy it to a mounted drive holding a file named `mark`.
    Drive { mark: Symbol },
    /// Run a program that writes it.
    Run(Invocation),
}

/// A program and its arguments (`std.build.Invocation`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Invocation {
    pub command: Symbol,
    pub args: Vec<Argument>,
}

/// An argument of an invocation (`std.build.Argument`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Argument {
    Text(Symbol),
    /// The file being given to the device.
    File,
}

impl Read for Method {
    fn read(value: &Value) -> Result<Self, Unread> {
        let Value::Variant { name, payload, .. } = value else { return Err(Unread::Record) };
        if *name == flash::dfu.symbol() {
            return Ok(Self::Dfu);
        }
        let [payload] = payload.as_slice() else { return Err(Unread::Record) };
        let m = Members::of(payload)?;
        if *name == flash::probe.symbol() {
            Ok(Self::Probe { chip: m.string(probe::chip)? })
        } else if *name == flash::uf2.symbol() {
            Ok(Self::Uf2 { family: m.int(uf2::family)?, deliver: m.list(uf2::deliver)? })
        } else {
            Err(Unread::Record)
        }
    }
}

impl Read for Delivery {
    fn read(value: &Value) -> Result<Self, Unread> {
        let Value::Variant { name, payload, .. } = value else { return Err(Unread::Record) };
        let [payload] = payload.as_slice() else { return Err(Unread::Record) };
        if *name == delivery::drive.symbol() {
            Ok(Self::Drive { mark: Members::of(payload)?.string(drive::mark)? })
        } else if *name == delivery::run.symbol() {
            Invocation::read(payload).map(Self::Run)
        } else {
            Err(Unread::Record)
        }
    }
}

impl Read for Invocation {
    fn read(value: &Value) -> Result<Self, Unread> {
        let m = Members::of(value)?;
        Ok(Self { command: m.string(invocation::command)?, args: m.list(invocation::args)? })
    }
}

impl Read for Argument {
    fn read(value: &Value) -> Result<Self, Unread> {
        let Value::Variant { name, payload, .. } = value else { return Err(Unread::Record) };
        if *name == argument::file.symbol() {
            Ok(Self::File)
        } else if let ([payload], true) = (payload.as_slice(), *name == argument::text.symbol()) {
            Members::text(payload).map(Self::Text).ok_or(Unread::Record)
        } else {
            Err(Unread::Record)
        }
    }
}

impl std::fmt::Display for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Drive { mark } => write!(f, "a mounted drive holding `{mark}`"),
            Self::Run(invocation) => write!(f, "`{}`", invocation.command),
        }
    }
}
