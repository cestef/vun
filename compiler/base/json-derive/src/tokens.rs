//! Paths into `vun_json`, and the values and errors the generated code builds from them.

use vun_derive_core::Tokens;

pub(crate) trait JsonTokens {
    /// A path in `vun_json`.
    fn json(self, name: &str) -> Self;
    /// `::vun_json::Trait::method(argument)`
    fn call(self, name: &str, method: &str, argument: Self) -> Self;
    /// `::vun_json::Mismatch::new(::vun_json::Expected::expected)`
    fn mismatch(self, expected: Self) -> Self;
    /// `::vun_json::Expected::name`
    fn expected(self, name: &str) -> Self;
    /// `::vun_json::Value::name`
    fn value(self, name: &str) -> Self;
}

impl JsonTokens for Tokens {
    fn json(self, name: &str) -> Self {
        self.path(&["vun_json", name])
    }

    fn call(self, name: &str, method: &str, argument: Self) -> Self {
        self.json(name).op("::").word(method).parens(argument)
    }

    fn mismatch(self, expected: Self) -> Self {
        self.json("Mismatch").op("::").word("new").parens(expected)
    }

    fn expected(self, name: &str) -> Self {
        self.json("Expected").op("::").word(name)
    }

    fn value(self, name: &str) -> Self {
        self.json("Value").op("::").word(name)
    }
}
