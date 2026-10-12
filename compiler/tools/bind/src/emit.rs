//! The second pass: each declaration bound, as made items, in the order the header declares
//! them.

use vun_clang::{Cursor, Kind, TypeKind};
use vun_syntax::case::Case;
use vun_syntax::make::{Bind, Expr, Field, File, Item, Member, Param};

use vun_binder::Library;

use crate::macros::{Names, Value};
use crate::paths;
use crate::types::Position;
use crate::{Binder, Opacity, Reason, items};

/// The field a struct bound as bytes holds them in, private to it.
const BYTES: &str = "_bytes";
/// The type parameter and the pack of a variadic function: the pack's type is inferred from the
/// arguments of each call (spec 07, section 5).
const PACK_TYPE: &str = "P";
const PACK: &str = "args";

/// An item without doc, attributes or type.
pub(crate) fn item(name: &str, value: Expr) -> Item {
    Bind::new(name, value).item()
}

/// An alias of the type `value`: annotated `type`, which is how a binding is known to bind one
/// (spec 10, section 5).
fn alias_of(name: &str, value: Expr) -> Item {
    item(name, value).typed(Expr::local("type"))
}

impl Binder<'_> {
    /// The Vun file binding `decls`.
    pub(crate) fn file(&mut self, decls: Vec<Cursor<'_>>, doc: Vec<String>) -> File {
        let mut made = Vec::new();
        for decl in decls {
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
        File { doc, spreads: Vec::new(), items: made }
    }

    /// The bindings' own top-level names, which parameters may not shadow.
    pub(crate) fn top_names(&self) -> std::collections::HashSet<String> {
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
                ..item(&name, Expr::Struct(Vec::new()))
            },
            Some(Opacity::Sized) => {
                let bytes =
                    Expr::ArrayType(Box::new(Expr::int(ty.size()?)), Box::new(items::integer(false, 8)));
                Item {
                    doc: vec![
                        "Bound as its size and alignment only: its fields are private to C.".to_owned(),
                    ],
                    attrs: vec![items::layout_c(), items::align(ty.align()?)],
                    ..item(&name, Expr::Struct(vec![Member::Field(Field::new(BYTES, bytes))]))
                }
            }
            None => {
                let fields = decl.children().into_iter().filter(|f| f.kind() == Kind::Field);
                let fields = fields
                    .map(|f| {
                        Some(Member::Field(Field::new(
                            &self.vun_name(&f.name(), Case::Snake),
                            self.ty(f.ty(), Position::Field).ok()??,
                        )))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Item { attrs: vec![items::layout_c()], ..item(&name, Expr::Struct(fields)) }
            }
        })
    }

    /// A C enum: an alias of its integer type, and a constant of that type for each value, since
    /// C code may hold any integer in it; an enum the request says C holds no other value in is a
    /// Vun enum (`closed`).
    fn enumeration(&self, decl: &Cursor<'_>) -> Vec<Item> {
        let Ok(Some(int)) = self.ty(decl.enum_type(), Position::Field) else { return Vec::new() };
        if let Some(made) = self.closed(decl, &int) {
            return made;
        }
        let mut made = Vec::new();
        let ty = match self.bound(decl) {
            Some(name) => {
                made.push(alias_of(&name, int));
                Expr::local(&name)
            }
            None => int,
        };
        for constant in decl.children().iter().filter(|c| c.kind() == Kind::EnumConstant) {
            let value = Expr::int(i128::from(constant.enum_value()));
            made.push(item(&self.vun_name(&constant.name(), Case::Snake), value).typed(ty.clone()));
        }
        made
    }

    fn alias(&mut self, decl: &Cursor<'_>) -> Option<Item> {
        let under = decl.underlying();
        let ty = if under.canonical().kind() == TypeKind::Function {
            self.function(under.canonical())
        } else {
            self.ty(under, Position::Alias).map(|t| t.unwrap_or(Expr::Unit))
        };
        match ty {
            Ok(ty) => Some(alias_of(&self.bound(decl)?, ty)),
            Err(why) => {
                self.skip(decl, Reason::Unsupported(why));
                None
            }
        }
    }

    /// `#[lang.link.symbol("f"), lang.abi.c] f: unsafe (...) -> R = _;`: calling C is unchecked.
    fn function_decl(&mut self, decl: &Cursor<'_>) -> Option<Item> {
        let variadic = decl.is_variadic();
        let declared = decl.params();
        let params = declared.iter().enumerate().map(|(i, param)| {
            let name = param.name();
            let name = if name.is_empty() { Self::positional(i) } else { self.vun_name(&name, Case::Snake) };
            let taken = self.params_reserved.contains(&name)
                || (variadic && [PACK, PACK_TYPE].contains(&name.as_str()));
            let name = if taken { self.renamed(&name) } else { name };
            Ok(Param::named(&name, self.ty(param.ty(), Position::Param)?.unwrap_or(Expr::Unit)))
        });
        let signature = params.collect::<Result<Vec<_>, _>>().and_then(|params| {
            // A variadic function takes the arguments past its parameters as a pack.
            let params = if variadic {
                let kind = Param::comptime(PACK_TYPE, Expr::local("type"));
                let rest = Param::pack(PACK, Expr::local(PACK_TYPE));
                [vec![kind], params, vec![rest]].concat()
            } else {
                params
            };
            Ok(items::signature(params, self.ty(decl.result(), Position::Result)?))
        });
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
        let library = self.library_of(decl).map(items::library);
        Item {
            attrs: [items::symbol(&symbol), items::abi_c()].into_iter().chain(library).collect(),
            ..item(&self.vun_name(&symbol, Case::Snake), Expr::Hole).typed(ty).mutable(mutable)
        }
    }

    /// The library `decl` is linked from: of those declaring its header, the one with the deepest
    /// directory.
    fn library_of(&self, decl: &Cursor<'_>) -> Option<&str> {
        let file = decl.location().file.map(|f| paths::real(&f))?;
        let declares = |l: &&Library| l.headers.as_ref().is_none_or(|dir| file.starts_with(dir));
        let depth = |l: &&Library| l.headers.as_ref().map_or(0, |dir| dir.components().count() + 1);
        self.libraries.iter().filter(declares).max_by_key(depth).map(|l| l.name.as_str())
    }

    fn constant(&self, decl: &Cursor<'_>) -> Option<Item> {
        let tokens = decl.tokens();
        let names = Names { macros: &self.macros, constants: &self.constants };
        let value = match names.value(tokens.get(1..)?)? {
            Value::Int(n) => Expr::int(n),
            Value::Float(x) => Expr::float(x),
            Value::Text(text) => Expr::CString(text),
        };
        Some(item(&self.vun_name(&decl.name(), Case::Snake), value))
    }
}
