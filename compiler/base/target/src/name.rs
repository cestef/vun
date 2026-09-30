//! Names of the open sets targets draw from.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

/// A name of one of the open sets targets draw from (decision 138): architectures, systems,
/// object formats, backends and linkers. Targets give theirs as `lang.meta.Name` values;
/// components declare theirs statically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(pub &'static str);

/// A name persists as a symbol, whose text lives as long as the process.
impl Encode for Name {
    fn encode(&self, w: &mut vun_codec::Writer<'_>) {
        vun_intern::Symbol::intern(self.0).encode(w);
    }
}

impl Decode for Name {
    fn decode(r: &mut vun_codec::Reader<'_>) -> Result<Self, vun_codec::Malformed> {
        vun_intern::Symbol::decode(r).map(|name| Self(name.as_str()))
    }
}

/// A name as data is its text, read back as a symbol, as it persists.
impl vun_data::ToData for Name {
    fn to_data(&self) -> vun_data::Value<'static> {
        vun_data::ToData::to_data(self.0)
    }
}

impl vun_data::FromData for Name {
    fn from_data(value: &vun_data::Value<'_>) -> Result<Self, vun_data::Mismatch> {
        <String as vun_data::FromData>::from_data(value).map(|text| Self(Symbol::intern(&text).as_str()))
    }
}

impl Name {
    /// Whether a target's name is this one.
    #[must_use]
    pub fn is(self, name: Symbol) -> bool {
        name.as_str() == self.0
    }

    #[must_use]
    pub fn symbol(self) -> Symbol {
        Symbol::intern(self.0)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
