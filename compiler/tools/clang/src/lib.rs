//! libclang, loaded when first needed and never linked in (decision 131): the C front end
//! `vun util ffi bind` reads headers with, through a safe API of what bindings need.
//!
//! This is one of the three places of the toolchain that use `unsafe` (the workspace denies it),
//! with `vun-par`'s pool and the component boundary: each call into libclang is wrapped
//! once, here. libclang is kept per thread by the loader, so a [`Clang`] and everything borrowed
//! from it stay on the thread that loaded it.

#![allow(unsafe_code)]
// libclang's constants keep their C names, which the kinds are matched against.
#![allow(non_upper_case_globals)]

mod cursor;
mod resource;
mod text;
mod ty;
mod unit;

use std::marker::PhantomData;

pub use cursor::{Cursor, Kind, Location};
pub use ty::{Type, TypeKind};
pub use unit::{Diagnostic, Severity, Unit};

/// The oldest libclang accepted, the major version the bindings are compiled against. The loader
/// tells versions apart by the functions a library has, which 9 and 10 share.
pub const OLDEST: u32 = 9;

/// libclang, loaded on this thread.
pub struct Clang {
    /// libclang lives in the loading thread's storage.
    _thread: PhantomData<*const ()>,
}

/// Why libclang could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// Not found or not openable, as the loader says.
    Unloadable(String),
    /// Older than [`OLDEST`], lacking functions the bindings call.
    Old,
}

impl Clang {
    /// Finds and opens libclang: where `LIBCLANG_PATH` names, then the system's usual places.
    ///
    /// # Errors
    /// Returns why no libclang of version [`OLDEST`] or later could be opened.
    pub fn load() -> Result<Self, Missing> {
        if !clang_sys::is_loaded() {
            clang_sys::load().map_err(Missing::Unloadable)?;
        }
        let version = clang_sys::get_library().and_then(|l| l.version());
        // The loader numbers a version as ten times its major one.
        if version.is_some_and(|v| (v as u32) < OLDEST * 10) {
            let _ = clang_sys::unload();
            return Err(Missing::Old);
        }
        Ok(Self { _thread: PhantomData })
    }
}

#[cfg(test)]
mod tests;
