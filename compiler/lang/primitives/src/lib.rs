//! The declarations of `lang` whose value is `_`, provided by the compiler, generated from
//! `lib/lang`: a module per namespace with an enum of its primitives, and `Primitive` over them.

#[rustfmt::skip]
mod generated;

use std::sync::LazyLock;

pub use generated::*;

impl Primitive {
    /// The primitive `item` is, as tables name them: `lang.mem.Ref`.
    #[must_use]
    pub fn at(item: vun_items::Item) -> Option<Self> {
        static BY_ITEM: LazyLock<foldhash::HashMap<vun_items::Item, Primitive>> =
            LazyLock::new(|| Primitive::all().map(|p| (p.item(), p)).collect());
        BY_ITEM.get(&item).copied()
    }
}

#[cfg(test)]
mod tests;

impl abi::Abi {
    /// Whether a function with it is called as C's are: records by value, a variadic tail. That is
    /// `#[abi.c]`, and each calling convention a target names.
    #[must_use]
    pub const fn is_c(self) -> bool {
        !matches!(self, Self::native | Self::interrupt | Self::naked)
    }

    /// Whether code outside the language can enter a function with it by its address: it is called
    /// as C's are, or it is `#[abi.naked]`, entered by what the target defines (a core taking an
    /// exception), which a table of addresses holds.
    #[must_use]
    pub const fn is_enterable(self) -> bool {
        self.is_c() || matches!(self, Self::naked)
    }

    /// The calling convention it names, when it names one rather than the target's own.
    #[must_use]
    pub const fn convention(self) -> Option<vun_target::Convention> {
        use vun_target::Convention;
        match self {
            Self::sysv => Some(Convention::Sysv),
            Self::win64 => Some(Convention::Win64),
            Self::cdecl => Some(Convention::Cdecl),
            Self::stdcall => Some(Convention::Stdcall),
            Self::fastcall => Some(Convention::Fastcall),
            Self::thiscall => Some(Convention::Thiscall),
            Self::native | Self::c | Self::interrupt | Self::naked => None,
        }
    }
}
