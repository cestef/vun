//! Paths into `vun_data`, and the values and errors the generated code builds from them.

use vun_derive_core::Tokens;

pub(crate) trait JsonTokens {
    /// A path in `vun_data`.
    fn data(self, name: &str) -> Self;
    /// `::vun_data::Trait::method(argument)`
    fn call(self, name: &str, method: &str, argument: Self) -> Self;
    /// `::vun_data::Mismatch::new(::vun_data::Expected::expected)`
    fn mismatch(self, expected: Self) -> Self;
    /// `::vun_data::Expected::name`
    fn expected(self, name: &str) -> Self;
    /// `::vun_data::Value::name`
    fn value(self, name: &str) -> Self;
}

impl JsonTokens for Tokens {
    fn data(self, name: &str) -> Self {
        self.path(&["vun_data", name])
    }

    fn call(self, name: &str, method: &str, argument: Self) -> Self {
        self.data(name).op("::").word(method).parens(argument)
    }

    fn mismatch(self, expected: Self) -> Self {
        self.data("Mismatch").op("::").word("new").parens(expected)
    }

    fn expected(self, name: &str) -> Self {
        self.data("Expected").op("::").word(name)
    }

    fn value(self, name: &str) -> Self {
        self.data("Value").op("::").word(name)
    }
}
