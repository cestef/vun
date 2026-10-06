//! Values that live as long as the process, and paths.

use std::path::PathBuf;

use crate::{Decode, Encode, Malformed, Reader, Writer};

/// A fact a component declares about itself (its names, its features) is read once and kept for
/// the process, as `&'static` data is: reading it leaks it, so it is for what is read a few
/// times, never per value.
impl Decode for &'static str {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        String::decode(r).map(|text| &*Box::leak(text.into_boxed_str()))
    }
}

impl<T: Decode> Decode for &'static [T] {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Vec::decode(r).map(|items| &*Vec::leak(items))
    }
}

/// A path persists as its text, a name that is not UTF-8 as its lossy reading.
impl Encode for PathBuf {
    fn encode(&self, w: &mut Writer<'_>) {
        self.to_string_lossy().encode(w);
    }
}

impl Decode for PathBuf {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        String::decode(r).map(Self::from)
    }
}
