//! The first pass: which declarations are bound, under which Vun names, and how much of each
//! struct. C keeps tags (`struct stat`) apart from ordinary names (`stat()`), Vun does not: a tag
//! that an ordinary name also spells is bound as `struct_stat`. A name the file sees already,
//! which Vun does not let a declaration shadow, takes a trailing `_`; a keyword is escaped by the
//! printer. Names are written in the case of what they bind, types in `PascalCase` and values in
//! `snake_case`; what the linker knows keeps its C name through `link.symbol`.

use std::collections::HashSet;

use vun_clang::{Cursor, Kind, TypeKind};
use vun_syntax::case::Case;

use crate::ctype::Position;
use crate::{Binder, Bytes, Key, Opacity, Reason};

/// What a name that would shadow another is marked with.
const RENAMED: &str = "_";

/// What a tag an ordinary name also spells is prefixed with, by the kind of its declaration.
fn tag_prefix(kind: Kind) -> &'static str {
    match kind {
        Kind::Union => "union_",
        Kind::Enum => "enum_",
        _ => "struct_",
    }
}

impl<'u> Binder<'u> {
    /// `name` written in `case`, the case of what it binds (spec 10, section 5), with `_` after
    /// it when the file sees it already.
    pub(crate) fn vun_name(&self, name: &str, case: Case) -> String {
        let name = case.write(name);
        if self.reserved.contains(&name) { self.renamed(&name) } else { name }
    }

    /// `name` with the mark that keeps it apart from a name it would shadow.
    pub(crate) fn renamed(&self, name: &str) -> String {
        let renamed = [name, RENAMED].concat();
        if self.reserved.contains(&renamed) { self.renamed(&renamed) } else { renamed }
    }

    /// Chooses what to bind of `mine`, the unit's declarations in the header's directory, and
    /// names each.
    pub(crate) fn collect(&mut self, mine: &[Cursor<'u>]) {
        // A typedef giving a struct its own name, `typedef struct p { ... } p;`, is that struct.
        let renames = |c: &Cursor<'_>| {
            let under = c.underlying();
            matches!(under.kind(), TypeKind::Record | TypeKind::Enum)
                && under.declaration().name() == c.name()
        };
        let ordinary: HashSet<String> = mine
            .iter()
            .flat_map(|c| match c.kind() {
                Kind::Typedef if renames(c) => Vec::new(),
                Kind::Function | Kind::Var | Kind::Typedef | Kind::Macro => vec![c.name()],
                Kind::Enum => c.children().iter().map(Cursor::name).collect(),
                _ => Vec::new(),
            })
            .collect();
        let mut seen: HashSet<Key> = HashSet::new();
        for decl in mine {
            if decl.kind() == Kind::Enum {
                for constant in decl.children().iter().filter(|c| c.kind() == Kind::EnumConstant) {
                    self.constants.insert(constant.name(), i128::from(constant.enum_value()));
                }
            }
            match decl.kind() {
                Kind::Struct | Kind::Union | Kind::Enum => self.tagged(decl, &ordinary, &mut seen),
                Kind::Typedef => self.typedef(decl),
                Kind::Function | Kind::Var if !decl.is_static() => {
                    if seen.insert(Key::Ordinary(decl.name())) {
                        self.decls.push(*decl);
                    }
                }
                Kind::Macro if !decl.is_function_like() => {
                    self.macros.insert(decl.name(), decl.tokens().split_off(1));
                    self.decls.push(*decl);
                }
                _ => {}
            }
        }
        self.opacity();
    }

    /// A struct, union or enum: bound once, at its definition when it has one.
    fn tagged(&mut self, decl: &Cursor<'u>, ordinary: &HashSet<String>, seen: &mut HashSet<Key>) {
        let Some(key) = Self::key(decl) else { return };
        if let Key::Tag(tag) = &key {
            let tag =
                if ordinary.contains(tag) { [tag_prefix(decl.kind()), tag].concat() } else { tag.clone() };
            let name = self.vun_name(&tag, Case::Pascal);
            self.names.entry(key.clone()).or_insert(name);
        }
        if seen.insert(key.clone()) {
            self.decls.push(*decl);
        } else if decl.is_definition()
            && let Some(slot) = self.decls.iter_mut().find(|d| Self::key(d).as_ref() == Some(&key))
        {
            *slot = *decl;
        }
    }

    /// A typedef names what it stands for: an anonymous struct takes its name, one of a struct
    /// of the same name is that struct, any other is an alias.
    fn typedef(&mut self, decl: &Cursor<'u>) {
        let name = decl.name();
        let under = decl.underlying();
        if matches!(under.kind(), TypeKind::Record | TypeKind::Enum)
            && let Some(key) = Self::key(&under.declaration())
        {
            match &key {
                Key::At(..) if !self.names.contains_key(&key) => {
                    self.names.insert(key, self.vun_name(&name, Case::Pascal));
                    self.names.insert(Key::Ordinary(name.clone()), self.vun_name(&name, Case::Pascal));
                    return;
                }
                Key::Tag(tag) if *tag == name => {
                    let bound =
                        self.names.get(&key).cloned().unwrap_or_else(|| self.vun_name(&name, Case::Pascal));
                    self.names.insert(Key::Ordinary(name), bound);
                    return;
                }
                _ => {}
            }
        }
        self.names.insert(Key::Ordinary(name), self.vun_name(&decl.name(), Case::Pascal));
        self.decls.push(*decl);
    }

    /// Decides, in source order, which structs are bound with their fields: those whose every
    /// field Vun can say, in a struct that is no union.
    fn opacity(&mut self) {
        let records: Vec<Cursor<'u>> =
            self.decls.iter().copied().filter(|d| matches!(d.kind(), Kind::Struct | Kind::Union)).collect();
        for record in records {
            let Some(name) = self.bound(&record) else { continue };
            let why = if !record.is_definition() {
                self.opaque.insert(name, Opacity::Incomplete);
                continue;
            } else if record.kind() == Kind::Union {
                Some(Bytes::Union)
            } else {
                record.children().iter().filter(|f| f.kind() == Kind::Field).find_map(|f| {
                    if f.is_bit_field() {
                        return Some(Bytes::BitField(f.name()));
                    }
                    self.ty(f.ty(), Position::Field).err().map(|_| Bytes::Field(f.name()))
                })
            };
            if let Some(why) = why {
                self.opaque.insert(name, Opacity::Sized);
                self.skip(&record, Reason::Bytes(why));
            }
        }
    }
}
