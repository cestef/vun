//! The forms of an architecture whose instructions are words of fixed fields (`vun_isa::word`), as
//! arm64's and Thumb's are: a base word, the widths a form is defined at with the bits each sets,
//! and its operand positions, spelled as the table's constructors build them.

use proc_macro2::TokenStream;
use quote::quote;
use vun_syntax::literal::Literal;

use super::emit::Crate;
use super::spell::{int, word};

/// A hole of a family's form, filled from `member`: its own base word, or the member's bits with
/// these set.
pub(super) fn fill<'s>(value: &Literal<'s>, member: &Literal<'s>) -> Option<Literal<'s>> {
    match value {
        Literal::Tagged("fixed", payload) => payload.first().cloned(),
        Literal::Tagged("or", payload) => match (payload.first(), member.get("bits")) {
            (Some(Literal::Int(bits)), Some(Literal::Int(own))) => Some(Literal::Int(own | bits)),
            _ => None,
        },
        _ => None,
    }
}

/// A form as a `table::Form` value, built by the table's constructors, a builder call for each
/// field its data sets.
pub(super) fn form(c: &Crate, record: &Literal<'_>) -> Option<TokenStream> {
    let field = |name| record.get(name);
    let base = word(field("base")?)?;
    let sizes = field("sizes")?.as_list()?.iter().map(|s| {
        let (size, bits) = (c.variant("Size", s.get("size")?)?, word(s.get("bits")?)?);
        let mut sized = quote! { table::Sized::new(#size, #bits) };
        if let Some(suffix) = s.get("suffix").and_then(Literal::as_str).filter(|s| !s.is_empty()) {
            sized = quote! { #sized.suffix(#suffix) };
        }
        let qualifiers = s.get("qualifiers").and_then(Literal::as_list).unwrap_or_default();
        if !qualifiers.is_empty() {
            let qualifiers = qualifiers.iter().map(Literal::as_str).collect::<Option<Vec<_>>>()?;
            sized = quote! { #sized.qualifiers(&[#(#qualifiers),*]) };
        }
        Some(sized)
    });
    let sizes = sizes.collect::<Option<Vec<_>>>()?;
    let args = field("args")?.as_list()?.iter().map(|a| {
        let (kind, at) = (c.value("Kind", a.get("kind")?)?, int::<u8>(a.get("at")?)?);
        let access = c.variant("Access", a.get("access")?)?;
        let arg = quote! { table::Arg::new(#kind, #at, #access) };
        Some(match a.get("again")? {
            Literal::Tagged("some", again) => {
                let again = int::<u8>(again.first()?)?;
                quote! { #arg.again(#again) }
            }
            _ => arg,
        })
    });
    let args = args.collect::<Option<Vec<_>>>()?;
    let mut form = quote! { table::Form::new(#base, &[#(#sizes),*], &[#(#args),*]) };
    if let Literal::Tagged("some", name) = field("name")? {
        let name = name.first()?.as_str()?;
        form = quote! { #form.named(#name) };
    }
    let (reads, writes) = (c.variants("Reg", field("reads")?)?, c.variants("Reg", field("writes")?)?);
    if !reads.is_empty() {
        form = quote! { #form.reads(&[#(#reads),*]) };
    }
    if !writes.is_empty() {
        form = quote! { #form.writes(&[#(#writes),*]) };
    }
    let apart = field("apart").and_then(Literal::as_list).unwrap_or_default();
    if !apart.is_empty() {
        let pairs = apart.iter().map(|p| {
            let [a, b] = p.as_list()? else { return None };
            let (a, b) = (int::<u8>(a)?, int::<u8>(b)?);
            Some(quote! { [#a, #b] })
        });
        let pairs = pairs.collect::<Option<Vec<_>>>()?;
        form = quote! { #form.apart(&[#(#pairs),*]) };
    }
    if let Some(Literal::Tagged("some", feature)) = field("requires") {
        let feature = c.variant("Feature", feature.first()?)?;
        form = quote! { #form.requires(#feature) };
    }
    if let Literal::Tagged("some", flags) = field("flags")? {
        let flags = c.variant("Access", flags.first()?)?;
        form = quote! { #form.flags(#flags) };
    }
    Some(form)
}
