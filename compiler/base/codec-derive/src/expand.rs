//! The implementations derived: fields in declaration order, after a variant's position.

use proc_macro::TokenStream;
use vun_derive_core::{Body, Item, Tokens};

/// The codec's implementations of an item.
pub(crate) trait Codec {
    fn encode(&self) -> TokenStream;
    fn decode(&self) -> TokenStream;
}

impl Codec for Item {
    fn encode(&self) -> TokenStream {
        let cases = self.cases();
        let arms = cases.iter().fold(Tokens::new(), |arms, case| {
            let tag = case.index.map(|tag| Tokens::new().encoding(Tokens::new().op("&").number(tag)));
            let fields = case
                .fields
                .all()
                .iter()
                .map(|f| Tokens::new().encoding(Tokens::new().tree(f.binding.clone())));
            let body = tag.into_iter().chain(fields).fold(Tokens::new(), Tokens::then);
            arms.then(case.pattern()).op("=>").braces(body)
        });
        // An enum without variants has no value to match a reference to.
        let scrutinee = if cases.is_empty() { Tokens::new().op("*") } else { Tokens::new() };
        let method = Tokens::new()
            .word("fn")
            .word("encode")
            .parens(
                Tokens::new()
                    .op("&")
                    .word("self")
                    .op(",")
                    .word("w")
                    .op(":")
                    .op("&")
                    .word("mut")
                    .codec_type("Writer"),
            )
            .braces(Tokens::new().word("match").then(scrutinee).word("self").braces(arms));
        self.implement(&Tokens::new().codec("Encode"), method)
    }

    fn decode(&self) -> TokenStream {
        let decoded = || std::iter::repeat_with(|| Tokens::new().decoding());
        let body = match &self.body {
            Body::Struct(_) => Tokens::new().ok(self.cases()[0].construct(decoded())),
            Body::Enum(_) => {
                let arms = self.cases().iter().fold(Tokens::new(), |arms, case| {
                    let tag = case.index.expect("a variant has a position");
                    arms.number(tag).op("=>").ok(case.construct(decoded())).op(",")
                });
                let tag = Tokens::new().word("tag");
                let malformed = Tokens::new().codec("Malformed").op("::").word("Tag");
                let widened = Tokens::new().path(&["core", "convert", "From", "from"]).parens(tag.clone());
                let unknown = Tokens::new().err(malformed.parens(widened));
                // The tags are `u32` literals, which type the value read.
                let read = Tokens::new().decoding();
                Tokens::new().word("match").then(read).braces(arms.then(tag).op("=>").then(unknown))
            }
        };
        let method = Tokens::new()
            .word("fn")
            .word("decode")
            .parens(Tokens::new().word("r").op(":").op("&").word("mut").codec_type("Reader"))
            .op("->")
            .result(Tokens::new().codec("Malformed"))
            .braces(body);
        self.implement(&Tokens::new().codec("Decode"), method)
    }
}

/// Paths and calls into `vun_codec`.
trait CodecTokens {
    /// A path in `vun_codec`.
    fn codec(self, name: &str) -> Self;
    /// `::vun_codec::Name<'_>`
    fn codec_type(self, name: &str) -> Self;
    /// `::vun_codec::Encode::encode(value, w);`
    fn encoding(self, value: Self) -> Self;
    /// `::vun_codec::Decode::decode(r)?`
    fn decoding(self) -> Self;
}

impl CodecTokens for Tokens {
    fn codec(self, name: &str) -> Self {
        self.path(&["vun_codec", name])
    }

    fn codec_type(self, name: &str) -> Self {
        self.codec(name).op("<").lifetime("_").op(">")
    }

    fn encoding(self, value: Self) -> Self {
        self.codec("Encode").op("::").word("encode").parens(value.op(",").word("w")).op(";")
    }

    fn decoding(self) -> Self {
        self.codec("Decode").op("::").word("decode").parens(Self::new().word("r")).op("?")
    }
}
