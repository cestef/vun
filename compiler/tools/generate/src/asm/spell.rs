//! Values of a table spelled as Rust: options, integers, words, and the registers a form uses
//! unnamed.

use proc_macro2::{Literal as Number, TokenStream};
use quote::{ToTokens, quote};
use vun_syntax::literal::Literal;

/// `.some(v)` as `Some(..)` of `v` read by `read`, `.none` as `None`.
pub(super) fn option<'s>(
    value: &Literal<'s>,
    read: impl Fn(&Literal<'s>) -> Option<TokenStream>,
) -> Option<TokenStream> {
    match value {
        Literal::Name("none") => Some(quote! { None }),
        Literal::Tagged("some", payload) => {
            let inner = read(payload.first()?)?;
            Some(quote! { Some(#inner) })
        }
        _ => None,
    }
}

/// An integer that fits `T`, as an unsuffixed literal.
pub(super) fn int<T: TryFrom<i128> + Into<u64>>(value: &Literal<'_>) -> Option<TokenStream> {
    let Literal::Int(n) = value else { return None };
    let n = Number::u64_unsuffixed(T::try_from(*n).ok()?.into());
    Some(quote! { #n })
}

/// A word, written in hexadecimal as the architecture's manual writes encodings, its halves
/// apart.
pub(super) fn word(value: &Literal<'_>) -> Option<TokenStream> {
    let Literal::Int(n) = value else { return None };
    let n = u32::try_from(*n).ok()?;
    Some(
        syn::LitInt::new(&format!("{:#06x}_{:04x}", n >> 16, n & 0xffff), proc_macro2::Span::call_site())
            .into_token_stream(),
    )
}

/// The general registers a form reads or writes without naming them, each once, reads first.
pub(super) fn implicit<'s>(reads: &[Literal<'s>], writes: &[Literal<'s>]) -> Vec<&'s str> {
    let mut regs: Vec<&str> = Vec::new();
    for reg in reads.iter().chain(writes).filter_map(Literal::as_name) {
        if !regs.contains(&reg) {
            regs.push(reg);
        }
    }
    regs
}
