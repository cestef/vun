//! The second pass: each declaration bound, as made items, in the order the header declares
//! them.

use vun_clang::{Cursor, Kind, TypeKind};
use vun_syntax::case::Case;
use vun_syntax::make::{Expr, Field, File, Item};

use crate::types::Position;
use crate::{Binder, Opacity, Reason, items, macros};

/// The field a struct bound as bytes holds them in, private to it.
const BYTES: &str = "_bytes";

/// An item without doc, attributes or type.
fn item(name: String, value: Expr) -> Item {
    Item { doc: Vec::new(), attrs: Vec::new(), mutable: false, name, ty: None, value }
}

/// An alias of the type `value`: annotated `type`, which is how a binding is known to bind one
/// (spec 10, section 5).
fn alias_of(name: String, value: Expr) -> Item {
    Item { ty: Some(Expr::local("type")), ..item(name, value) }
}

impl Binder<'_> {
    /// The Vun file binding the header named `header`.
    pub(crate) fn file(&mut self, header: &str) -> File {
        let doc = vec![
            ["Bindings to the C header `", header, "`, written by `vun ffi bind`: regenerate them"].concat(),
            "rather than edit them.".to_owned(),
        ];
        self.params_reserved = self.top_names();
        let mut made = Vec::new();
        for decl in self.decls.clone() {
            match decl.kind() {
                Kind::Struct | Kind::Union => made.extend(self.record(&decl)),
                Kind::Enum => made.extend(self.enumeration(&decl)),
                Kind::Typedef => made.extend(self.alias(&decl)),
                Kind::Function => made.extend(self.function_decl(&decl)),
                Kind::Var => made.extend(self.variable(&decl)),
                Kind::Macro => made.extend(self.constant(&decl)),
                _ => {}
            }
        }
        File { doc, items: made }
    }

    /// The bindings' own top-level names, which parameters may not shadow.
    fn top_names(&self) -> std::collections::HashSet<String> {
        let ordinary = self.decls.iter().filter_map(|d| match d.kind() {
            Kind::Function | Kind::Var | Kind::Macro => Some(self.vun_name(&d.name(), Case::Snake)),
            _ => None,
        });
        let constants = self.decls.iter().filter(|d| d.kind() == Kind::Enum).flat_map(Cursor::children);
        self.names
            .values()
            .cloned()
            .chain(ordinary)
            .chain(constants.map(|c| self.vun_name(&c.name(), Case::Snake)))
            .collect()
    }

    fn record(&self, decl: &Cursor<'_>) -> Option<Item> {
        let name = self.bound(decl)?;
        let ty = decl.ty();
        Some(match self.opaque.get(&name) {
            Some(Opacity::Incomplete) => Item {
                doc: vec!["Declared without fields in C: reached through pointers only.".to_owned()],
                ..item(name, Expr::Struct(Vec::new()))
            },
            Some(Opacity::Sized) => {
                let bytes = Expr::Array(
                    Box::new(Expr::Int(i128::from(ty.size()?))),
                    Box::new(items::integer(false, 8)),
                );
                Item {
                    doc: vec![
                        "Bound as its size and alignment only: its fields are private to C.".to_owned(),
                    ],
                    attrs: vec![items::layout_c(), items::align(ty.align()?)],
                    ..item(name, Expr::Struct(vec![Field { name: BYTES.to_owned(), ty: bytes }]))
                }
            }
            None => {
                let fields = decl.children().into_iter().filter(|f| f.kind() == Kind::Field);
                let fields = fields
                    .map(|f| {
                        Some(Field {
                            name: self.vun_name(&f.name(), Case::Snake),
                            ty: self.ty(f.ty(), Position::Field).ok()??,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Item { attrs: vec![items::layout_c()], ..item(name, Expr::Struct(fields)) }
            }
        })
    }

    /// A C enum: an alias of its integer type, and a constant of that type for each value, since
    /// C code may hold any integer in it.
    fn enumeration(&self, decl: &Cursor<'_>) -> Vec<Item> {
        let Ok(Some(int)) = self.ty(decl.enum_type(), Position::Field) else { return Vec::new() };
        let mut made = Vec::new();
        let ty = match self.bound(decl) {
            Some(name) => {
                made.push(alias_of(name.clone(), int));
                Expr::local(&name)
            }
            None => int,
        };
        for constant in decl.children().iter().filter(|c| c.kind() == Kind::EnumConstant) {
            let value = Expr::Int(i128::from(constant.enum_value()));
            made.push(Item {
                ty: Some(ty.clone()),
                ..item(self.vun_name(&constant.name(), Case::Snake), value)
            });
        }
        made
    }

    fn alias(&mut self, decl: &Cursor<'_>) -> Option<Item> {
        let under = decl.underlying();
        let ty = if under.canonical().kind() == TypeKind::Function {
            self.function(under.canonical())
        } else {
            self.ty(under, Position::Field).map(|t| t.unwrap_or(Expr::Unit))
        };
        match ty {
            Ok(ty) => Some(alias_of(self.bound(decl)?, ty)),
            Err(why) => {
                self.skip(decl, Reason::Unsupported(why));
                None
            }
        }
    }

    /// `#[lang.link.symbol("f"), lang.abi.c] f: unsafe (...) -> R = _;`: calling C is unchecked.
    fn function_decl(&mut self, decl: &Cursor<'_>) -> Option<Item> {
        if decl.is_variadic() {
            self.skip(decl, Reason::Variadic);
            return None;
        }
        let declared = decl.params();
        let params = declared.iter().enumerate().map(|(i, param)| {
            let name = param.name();
            let name = if name.is_empty() { Self::positional(i) } else { self.vun_name(&name, Case::Snake) };
            let name = if self.params_reserved.contains(&name) { self.renamed(&name) } else { name };
            Ok(Field { name, ty: self.ty(param.ty(), Position::Param)?.unwrap_or(Expr::Unit) })
        });
        let signature = params
            .collect::<Result<Vec<_>, _>>()
            .and_then(|params| Ok(Expr::Fn(params, self.ty(decl.result(), Position::Result)?.map(Box::new))));
        match signature {
            Ok(signature) => Some(self.linked(decl, false, items::unchecked(signature))),
            Err(why) => {
                self.skip(decl, Reason::Unsupported(why));
                None
            }
        }
    }

    /// An `extern` variable: a static C may change, unless it is `const`.
    fn variable(&mut self, decl: &Cursor<'_>) -> Option<Item> {
        match self.ty(decl.ty(), Position::Field) {
            Ok(ty) => Some(self.linked(decl, !decl.ty().is_const(), ty?)),
            Err(why) => {
                self.skip(decl, Reason::Unsupported(why));
                None
            }
        }
    }

    /// `decl`, provided by the linker under its C name.
    fn linked(&self, decl: &Cursor<'_>, mutable: bool, ty: Expr) -> Item {
        let symbol = decl.name();
        Item {
            attrs: vec![items::symbol(&symbol), items::abi_c()],
            mutable,
            ty: Some(ty),
            ..item(self.vun_name(&symbol, Case::Snake), Expr::Hole)
        }
    }

    fn constant(&self, decl: &Cursor<'_>) -> Option<Item> {
        let tokens = decl.tokens();
        let value = macros::integer(tokens.get(1..)?)?;
        Some(item(self.vun_name(&decl.name(), Case::Snake), Expr::Int(value)))
    }
}
