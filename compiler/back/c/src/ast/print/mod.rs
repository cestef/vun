//! The one writer of C text: types, declarators, expressions and statements, with every nested
//! compound expression parenthesized so no precedence rule decides a meaning.

mod expr;
mod stmt;

use std::fmt::{self, Write};

use super::{Aggregate, Base, Decl, File, Ident, Item, Names, Own, Proto, Storage, Ty};

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

    /// A type as a cast writes it.
    fn ty(&mut self, ty: &Ty) -> fmt::Result {
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
            Ty::Float => self.f.write_str("float"),
            Ty::Double => self.f.write_str("double"),
            Ty::Pointer(to) => {
                self.ty(to)?;
                self.f.write_str(if matches!(**to, Ty::Pointer(_)) { "*" } else { " *" })
            }
            Ty::Restrict(pointer) => {
                self.ty(pointer)?;
                self.f.write_str("restrict")
            }
            Ty::Volatile(ty) => {
                self.f.write_str("volatile ")?;
                self.ty(ty)
            }
            Ty::Struct(tag) => {
                self.f.write_str("struct ")?;
                self.ident(*tag)
            }
            Ty::Union(tag) => {
                self.f.write_str("union ")?;
                self.ident(*tag)
            }
            Ty::Function { result, params } => {
                self.ty(result)?;
                self.f.write_str(" (*)")?;
                self.params(params.iter(), Self::ty)
            }
        }
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
        if let Ty::Function { result, params } = &decl.ty {
            self.ty(result)?;
            self.f.write_str(" (*")?;
            self.ident(decl.name)?;
            self.f.write_char(')')?;
            return self.params(params.iter(), Self::ty);
        }
        self.ty(&decl.ty)?;
        if !matches!(decl.ty, Ty::Pointer(_)) {
            self.f.write_char(' ')?;
        }
        self.ident(decl.name)?;
        match decl.len {
            Some(len) => write!(self.f, "[{len}]"),
            None => Ok(()),
        }
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
        self.params(proto.params.iter(), Self::decl)
    }

    fn item(&mut self, item: &Item) -> fmt::Result {
        match item {
            Item::Include(header) => writeln!(self.f, "#include \"{header}\""),
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
            Item::Declare(proto) => {
                self.proto(proto)?;
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
            Item::Object { storage, constant, decl, init } => {
                self.storage(*storage)?;
                if *constant {
                    self.f.write_str("const ")?;
                }
                self.decl(decl)?;
                if let Some(init) = init {
                    self.f.write_str(" = ")?;
                    self.expr(init)?;
                }
                self.f.write_str(";\n")
            }
        }
    }
}
