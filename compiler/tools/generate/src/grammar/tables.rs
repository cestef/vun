//! Tables: lists whose element type is inferred from their values.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use vun_diag::Msg;
use vun_syntax::literal::{Decl, Literal};

use super::{Enum, GENERATOR, Grammar, SHAPE, Ty, UNRESOLVED};
use crate::generate::{Access, Module, Read, Source};
use crate::items;

use crate::names::{Name, constant, docs, ident, pascal, raw};

/// Ends the name of a table's row struct: `iteration` holds `IterationRow`s.
const ROW: &str = "_row";

impl<'s> Grammar<'s> {
    /// A module per enum, per kind's children and per table declared in `own`, named after the
    /// declaration, each importing `uses` too.
    pub(super) fn emit(
        &self,
        source: &Source<'s>,
        own: &HashSet<&str>,
        uses: &TokenStream,
    ) -> Read<Vec<Module>> {
        let mut modules = Vec::new();
        for e in self.enums.iter().filter(|e| own.contains(e.name.as_ref())) {
            let (items, names) = e.emit();
            modules.push(Module::sibling(e.name.snake(), &quote! { #uses #items }));
            modules.push(Module {
                access: Access::Impl,
                ..Module::sibling(format!("{}_names", e.name.snake()), &quote! { #uses #names })
            });
            modules.extend(self.slots(e, &source.decls)?);
        }
        for d in source.decls.iter().filter(|d| own.contains(d.name)) {
            if let Some(items) = d.value.as_list() {
                let table = self.table(d, items)?;
                modules.push(Module::sibling(d.name.to_owned(), &quote! { #uses #table }));
            }
        }
        Ok(modules)
    }

    /// A table: a list of rows. Rows that are records of several fields are values of a struct
    /// named after the table (`iteration` holds `IterationRow`s), read by field name.
    pub(super) fn table(&self, d: &Decl<'s>, items: &[Literal<'s>]) -> Read<TokenStream> {
        let ty = self.infer(d.name, &items.iter().collect::<Vec<_>>())?;
        let (name, doc) = (constant(d.name), docs(&d.doc));
        if let Ty::Tuple(fields) = &ty
            && fields.len() > 1
        {
            let row = pascal(&[d.name, ROW].concat());
            let names: Vec<_> = fields.iter().map(|(field, _)| raw(field)).collect();
            let types = fields.iter().map(|(_, t)| self.rust_type(t));
            let rows = items.iter().map(|item| {
                let values = fields.iter().filter_map(|(field, t)| Some(self.value(item.get(field)?, t)));
                quote! { #row { #(#names: #values),* } }
            });
            let row_doc = docs(&[&format!("A row of [`{name}`].")]);
            return Ok(quote! {
                #(#[doc = #row_doc])*
                #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                pub struct #row { #(pub #names: #types),* }
                #(#[doc = #doc])* pub const #name: &[#row] = &[#(#rows),*];
            });
        }
        // A constant's references are `'static` already.
        let rust_ty = match &ty {
            Ty::List(item) => {
                let item = self.rust_type(item);
                quote! { &[#item] }
            }
            _ => self.rust_type(&ty),
        };
        let values = items.iter().map(|item| self.value(item, &ty));
        Ok(quote! { #(#[doc = #doc])* pub const #name: &[#rust_ty] = &[#(#values),*]; })
    }

    /// The one type of all `values`.
    pub(super) fn infer(&self, decl: &str, values: &[&Literal<'s>]) -> Read<Ty> {
        let mismatch =
            || GENERATOR.error(SHAPE, decl, Msg::new().text("the values of a table must share one shape"));
        match values.first() {
            None => Err(GENERATOR.error(SHAPE, decl, Msg::new().text("an empty list has no type to infer"))),
            Some(Literal::String(_)) if values.iter().all(|v| v.as_str().is_some()) => Ok(Ty::Str),
            Some(Literal::Path(_)) if values.iter().all(|v| matches!(v, Literal::Path(_))) => Ok(Ty::Item),
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
            Some(Literal::Record(first))
                if values.iter().all(|v| Self::fields(v) == Self::fields(values[0])) =>
            {
                first
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
                    .map(Ty::Tuple)
            }
            Some(_) => Err(mismatch()),
        }
    }

    /// The names of a record's fields, in order; none for another value.
    fn fields<'v>(value: &'v Literal<'s>) -> Vec<&'v str> {
        match value {
            Literal::Record(fields) => fields.iter().map(|f| f.name).collect(),
            _ => Vec::new(),
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
                let candidates = found.iter().map(|&i| self.enums[i].name.as_ref());
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
            Ty::Str => quote! { &'static str },
            Ty::Item => quote! { vun_items::Item },
            Ty::Enum(i) => ident(&self.enums[*i].name).into_token_stream(),
            Ty::List(item) => {
                let item = self.rust_type(item);
                quote! { &'static [#item] }
            }
            // A record of one field is the field.
            Ty::Tuple(fields) if let [(_, field)] = fields.as_slice() => self.rust_type(field),
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
            (Ty::Item, Literal::Path(path)) => items::Handles::rust_path(path),
            (Ty::Enum(i), Literal::Name(name)) => {
                let (e, v) = (ident(&self.enums[*i].name), pascal(name));
                quote! { #e::#v }
            }
            (Ty::List(item), Literal::List(items)) => {
                let items = items.iter().map(|v| self.value(v, item));
                quote! { &[#(#items),*] }
            }
            // A record of one field is the field.
            (Ty::Tuple(fields), Literal::Record(_)) if let [(name, t)] = fields.as_slice() => {
                value.get(name).map(|v| self.value(v, t)).unwrap_or_default()
            }
            (Ty::Tuple(fields), Literal::Record(_)) => {
                let values = fields.iter().filter_map(|(name, t)| Some(self.value(value.get(name)?, t)));
                quote! { (#(#values),*) }
            }
            _ => unreachable!("infer checked every value against its table's type"),
        }
    }
}
