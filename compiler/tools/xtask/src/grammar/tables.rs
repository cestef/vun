//! Tables: lists whose element type is inferred from their values.

use overt_diag::Msg;
use overt_syntax::literal::{Decl, Literal};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::{Enum, GENERATOR, Grammar, SHAPE, Ty, UNRESOLVED};
use crate::generate::{Module, Read, Source};
use crate::names::{constant, docs, ident, pascal, snake};

impl<'s> Grammar<'s> {
    /// A module per enum, per kind's children and per table, named after the declaration.
    pub(super) fn emit(&self, source: &Source<'s>) -> Read<Vec<Module>> {
        let mut modules = Vec::new();
        for e in &self.enums {
            modules.push(Module::sibling(snake(e.name), &e.emit()));
            modules.extend(self.slots(e)?);
        }
        for d in &source.decls {
            if let Some(items) = d.value.as_list() {
                modules.push(Module::sibling(d.name.to_owned(), &self.table(d, items)?));
            }
        }
        Ok(modules)
    }

    pub(super) fn table(&self, d: &Decl<'s>, items: &[Literal<'s>]) -> Read<TokenStream> {
        let ty = self.infer(d.name, &items.iter().collect::<Vec<_>>())?;
        let rust_ty = self.rust_type(&ty);
        let values = items.iter().map(|item| self.value(item, &ty));
        let (name, doc) = (constant(d.name), docs(&d.doc));
        Ok(quote! { #(#[doc = #doc])* pub const #name: &[#rust_ty] = &[#(#values),*]; })
    }

    /// The one type of all `values`.
    pub(super) fn infer(&self, decl: &str, values: &[&Literal<'s>]) -> Read<Ty> {
        let mismatch =
            || GENERATOR.error(SHAPE, decl, Msg::new().text("the values of a table must share one shape"));
        match values.first() {
            None => Err(GENERATOR.error(SHAPE, decl, Msg::new().text("an empty list has no type to infer"))),
            Some(Literal::String(_)) if values.iter().all(|v| v.as_str().is_some()) => Ok(Ty::Str),
            Some(Literal::Name(_)) => {
                let names =
                    values.iter().map(|v| v.as_name()).collect::<Option<Vec<_>>>().ok_or_else(mismatch)?;
                self.resolve(decl, &names).map(Ty::Enum)
            }
            Some(Literal::List(_)) => {
                let lists =
                    values.iter().map(|v| v.as_list()).collect::<Option<Vec<_>>>().ok_or_else(mismatch)?;
                let items: Vec<_> = lists.into_iter().flatten().collect();
                self.infer(decl, &items).map(|t| Ty::List(Box::new(t)))
            }
            Some(Literal::Record(first)) => first
                .iter()
                .map(|field| {
                    let column = values
                        .iter()
                        .map(|v| v.get(field.name))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(mismatch)?;
                    Ok((field.name.to_owned(), self.infer(decl, &column)?))
                })
                .collect::<Read<_>>()
                .map(Ty::Tuple),
            Some(_) => Err(mismatch()),
        }
    }

    /// The one enum that has every name in `names`.
    pub(super) fn resolve(&self, decl: &str, names: &[&str]) -> Read<usize> {
        let has_all = |e: &Enum<'_>| names.iter().all(|n| e.variants.iter().any(|v| v.name == *n));
        let found: Vec<_> = (0..self.enums.len()).filter(|&i| has_all(&self.enums[i])).collect();
        match found[..] {
            [one] => Ok(one),
            [] => {
                let names = names.iter().map(|n| format!(".{n}"));
                Err(GENERATOR.error(
                    UNRESOLVED,
                    decl,
                    Msg::new().text("no kind or enum has all of ").code_list(names),
                ))
            }
            _ => {
                let candidates = found.iter().map(|&i| self.enums[i].name);
                Err(GENERATOR.error(
                    UNRESOLVED,
                    decl,
                    Msg::new().text("the names fit several enums: ").code_list(candidates),
                ))
            }
        }
    }

    pub(super) fn rust_type(&self, ty: &Ty) -> TokenStream {
        match ty {
            Ty::Str => quote! { &str },
            Ty::Enum(i) => ident(self.enums[*i].name).into_token_stream(),
            Ty::List(item) => {
                let item = self.rust_type(item);
                quote! { &[#item] }
            }
            Ty::Tuple(fields) => {
                let fields = fields.iter().map(|(_, t)| self.rust_type(t));
                quote! { (#(#fields),*) }
            }
        }
    }

    /// A value of type `ty`, whose shape `infer` checked.
    pub(super) fn value(&self, value: &Literal<'_>, ty: &Ty) -> TokenStream {
        match (ty, value) {
            (Ty::Str, Literal::String(text)) => quote! { #text },
            (Ty::Enum(i), Literal::Name(name)) => {
                let (e, v) = (ident(self.enums[*i].name), pascal(name));
                quote! { #e::#v }
            }
            (Ty::List(item), Literal::List(items)) => {
                let items = items.iter().map(|v| self.value(v, item));
                quote! { &[#(#items),*] }
            }
            (Ty::Tuple(fields), Literal::Record(_)) => {
                let values = fields.iter().filter_map(|(name, t)| Some(self.value(value.get(name)?, t)));
                quote! { (#(#values),*) }
            }
            _ => unreachable!("infer checked every value against its table's type"),
        }
    }
}
