//! The one writer of C text: types, declarators, expressions and statements, with every nested
//! compound expression parenthesized so no precedence rule decides a meaning.

mod asm;
mod expr;
mod stmt;

use std::fmt::{self, Write};

use vun_backend::Emits;

use super::{Aggregate, Attribute, Base, Decl, Duration, File, Ident, Item, Names, Own, Proto, Storage, Ty};

struct Printer<'f, 'a, 'n> {
    f: &'f mut fmt::Formatter<'a>,
    names: &'n Names,
    depth: usize,
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut p = Printer { f, names: &self.names, depth: 0 };
        for (i, item) in self.items.iter().enumerate() {
            let group = |item: &Item| std::mem::discriminant(item);
            if i > 0 && (group(item) != group(&self.items[i - 1]) || matches!(item, Item::Define(..))) {
                p.f.write_char('\n')?;
            }
            p.item(item)?;
        }
        Ok(())
    }
}

impl Printer<'_, '_, '_> {
    fn ident(&mut self, ident: Ident) -> fmt::Result {
        self.f.write_str(&self.names.spell(ident))
    }

    /// A type as a cast writes it: its declarator with no name.
    fn ty(&mut self, ty: &Ty) -> fmt::Result {
        self.left(ty)?;
        self.right(ty)
    }

    /// What a declarator writes left of its name: the specifier, then pointers and their
    /// qualifiers, inner ones first, so `u8 *volatile *` is a pointer to a volatile pointer.
    fn left(&mut self, ty: &Ty) -> fmt::Result {
        match ty {
            Ty::Void => self.f.write_str("void"),
            Ty::Base { base, signed } => {
                self.f.write_str(if *signed { "signed " } else { "unsigned " })?;
                self.f.write_str(match base {
                    Base::Char => "char",
                    Base::Short => "short",
                    Base::Int => "int",
                    Base::Long => "long",
                    Base::LongLong => "long long",
                })
            }
            &Ty::Int { bits, signed } => self.ident(Ident::Own(Own::Int { bits, signed })),
            Ty::Wide { signed } => self.f.write_str(if *signed { "__int128" } else { "unsigned __int128" }),
            Ty::Float => self.f.write_str("float"),
            Ty::Double => self.f.write_str("double"),
            Ty::Struct(tag) => {
                self.f.write_str("struct ")?;
                self.ident(*tag)
            }
            Ty::Union(tag) => {
                self.f.write_str("union ")?;
                self.ident(*tag)
            }
            Ty::Volatile(ty) if !ty.derived() => {
                self.f.write_str("volatile ")?;
                self.left(ty)
            }
            Ty::Volatile(pointer) => self.qualified(pointer, "volatile"),
            Ty::Restrict(pointer) => self.qualified(pointer, "restrict"),
            Ty::Pointer(to) => {
                self.left(to)?;
                self.gap(to)?;
                self.f.write_char('*')
            }
            Ty::Function { result, .. } => {
                self.left(result)?;
                self.gap(result)?;
                self.f.write_str("(*")
            }
        }
    }

    /// What a declarator writes right of its name: a function pointer's parameters.
    fn right(&mut self, ty: &Ty) -> fmt::Result {
        match ty {
            Ty::Volatile(ty) | Ty::Restrict(ty) | Ty::Pointer(ty) => self.right(ty),
            Ty::Function { result, params, variadic } => {
                self.f.write_char(')')?;
                self.declared(params.iter(), Self::ty, *variadic)?;
                self.right(result)
            }
            _ => Ok(()),
        }
    }

    /// A qualifier on a pointer, written after its `*`.
    fn qualified(&mut self, pointer: &Ty, qualifier: &str) -> fmt::Result {
        self.left(pointer)?;
        self.f.write_str(qualifier)
    }

    /// The space between `ty`'s left part and what follows it, unless it ends in `*` or `(*`.
    fn gap(&mut self, ty: &Ty) -> fmt::Result {
        if matches!(ty, Ty::Pointer(_) | Ty::Function { .. }) { Ok(()) } else { self.f.write_char(' ') }
    }

    /// A function's parameters as declared: `(a, b)`, `(a, b, ...)` when `variadic`, or
    /// `(void)` for none.
    fn declared<T>(
        &mut self,
        items: impl ExactSizeIterator<Item = T>,
        mut each: impl FnMut(&mut Self, T) -> fmt::Result,
        variadic: bool,
    ) -> fmt::Result {
        if !variadic {
            return self.params(items, each);
        }
        self.f.write_char('(')?;
        for item in items {
            each(self, item)?;
            self.f.write_str(", ")?;
        }
        self.f.write_str("...)")
    }

    /// `(a, b)`, or `(void)` for none.
    fn params<T>(
        &mut self,
        items: impl ExactSizeIterator<Item = T>,
        mut each: impl FnMut(&mut Self, T) -> fmt::Result,
    ) -> fmt::Result {
        if items.len() == 0 {
            return self.f.write_str("(void)");
        }
        self.f.write_char('(')?;
        for (i, item) in items.enumerate() {
            if i > 0 {
                self.f.write_str(", ")?;
            }
            each(self, item)?;
        }
        self.f.write_char(')')
    }

    fn decl(&mut self, decl: &Decl) -> fmt::Result {
        self.left(&decl.ty)?;
        self.gap(&decl.ty)?;
        self.ident(decl.name)?;
        if let Some(len) = decl.len {
            write!(self.f, "[{len}]")?;
        }
        self.right(&decl.ty)
    }

    /// ` __attribute__((aligned(16), section(".vectors")))`, or nothing without attributes.
    fn attributes(&mut self, attributes: &[Attribute]) -> fmt::Result {
        if attributes.is_empty() {
            return Ok(());
        }
        self.f.write_str(" __attribute__((")?;
        for (k, attribute) in attributes.iter().enumerate() {
            if k > 0 {
                self.f.write_str(", ")?;
            }
            match attribute {
                Attribute::Aligned(bytes) => write!(self.f, "aligned({bytes})")?,
                Attribute::Section(name) => write!(self.f, "section(\"{}\")", name.as_str())?,
                Attribute::Import { module, name } => write!(
                    self.f,
                    "import_module(\"{}\"), import_name(\"{}\")",
                    module.as_str(),
                    name.as_str()
                )?,
                Attribute::Used => self.f.write_str("used")?,
                Attribute::Naked => self.f.write_str("naked")?,
            }
        }
        self.f.write_str("))")
    }

    fn storage(&mut self, storage: Storage) -> fmt::Result {
        self.f.write_str(match storage {
            Storage::Static => "static ",
            Storage::Extern => "extern ",
            Storage::Public => "",
        })
    }

    fn proto(&mut self, proto: &Proto) -> fmt::Result {
        self.storage(proto.storage)?;
        self.decl(&Decl::new(proto.result.clone(), proto.name))?;
        self.declared(proto.params.iter(), Self::decl, proto.variadic)
    }

    fn item(&mut self, item: &Item) -> fmt::Result {
        match item {
            Item::Include(part) => writeln!(self.f, "#include \"{}\"", part.file(Emits::Header).display()),
            Item::Typedef(decl) => {
                self.f.write_str("typedef ")?;
                self.decl(decl)?;
                self.f.write_str(";\n")
            }
            Item::Aggregate { kind, tag, fields } => {
                self.ty(&match kind {
                    Aggregate::Struct => Ty::Struct(*tag),
                    Aggregate::Union => Ty::Union(*tag),
                })?;
                self.f.write_str(" {\n")?;
                for field in fields {
                    self.f.write_str("    ")?;
                    self.decl(field)?;
                    self.f.write_str(";\n")?;
                }
                self.f.write_str("};\n")
            }
            Item::Check { name, holds } => {
                self.f.write_str("typedef char ")?;
                self.ident(*name)?;
                self.f.write_char('[')?;
                self.operand(holds)?;
                self.f.write_str(" ? 1 : -1];\n")
            }
            Item::Declare(proto, attributes) => {
                self.proto(proto)?;
                self.attributes(attributes)?;
                self.f.write_str(";\n")
            }
            Item::Define(proto, body) => {
                self.proto(proto)?;
                self.f.write_str(" {\n")?;
                self.depth = 1;
                self.stmts(body)?;
                self.depth = 0;
                self.f.write_str("}\n")
            }
            Item::Object { storage, duration, constant, decl, attributes, init } => {
                self.storage(*storage)?;
                if *duration == Duration::Thread {
                    self.f.write_str("__thread ")?;
                }
                if *constant {
                    self.f.write_str("const ")?;
                }
                self.decl(decl)?;
                self.attributes(attributes)?;
                if let Some(init) = init {
                    self.f.write_str(" = ")?;
                    self.expr(init)?;
                }
                self.f.write_str(";\n")
            }
        }
    }
}
