//! The assemblers' tables, from `lib/lang/lang/arch` (spec 07, section 6): each architecture's
//! instructions as declared there, families expanded, become its `Op` enum and the forms of each.
//!
//! An architecture `a` declares its types and registers in `arch/a.vun` and its instructions in
//! `arch/a/*.vun`, where other types of it may be declared too. An instruction is a value of
//! `Instruction`, or a `Member` of a `Family` declared before it in its file, whose forms hold
//! holes the member fills; a `Mnemonic` names the instructions it spells. A form's absent fields
//! take the defaults `Form` declares, and a name starting with `_` is a shorthand of its file, read
//! as the value it names.

mod arm64;
mod emit;
mod ops;
mod spell;
mod thumb;
mod wasm32;
mod words;
mod x86_64;

use std::collections::BTreeMap;

use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Literal};

use crate::generate::{Generator, Read, Source};
use crate::names::docs;

pub(crate) use arm64::ARM64;
pub(crate) use thumb::THUMB;
pub(crate) use wasm32::WASM32;
pub(crate) use x86_64::X86_64;

/// Where the architectures are declared, relative to the repository root.
const ARCH: &str = "lib/lang/lang/arch";

/// The declarations whose values are instructions, families, their members and mnemonics, by the
/// last segment of their type's path, and the type whose field defaults absent fields take.
const INSTRUCTION: &str = "Instruction";
const FAMILY: &str = "Family";
const FORM: &str = "Form";
const MEMBER: &str = "Member";
const MNEMONIC: &str = "Mnemonic";
/// The fields of an instruction or a family, of a member, and of a mnemonic.
const FORMS: &str = "forms";
const OF_FAMILY: &str = "family";
const INSTRUCTIONS: &str = "instructions";
/// The fields of a form holding lists of records, and the type of those records, whose defaults
/// they take too.
const NESTED: [(&str, &str); 3] = [("operands", "Operand"), ("sizes", "Sized"), ("args", "Arg")];

/// A name that is a shorthand of its file rather than a declaration of the architecture.
const PRIVATE: char = '_';

const UNREAD: DiagId = DiagId::new("generate.asm.unread");

/// An instruction: its name, its mnemonic where a `Mnemonic` spells it (`b` for `bcond`), its doc
/// lines, and its forms, each field present.
pub(crate) struct Instruction<'s> {
    pub(crate) name: &'s str,
    pub(crate) mnemonic: Option<String>,
    pub(crate) doc: Vec<String>,
    pub(crate) forms: Vec<Literal<'s>>,
}

/// An architecture as its files declare it: its own declarations, types and registers, and its
/// instructions, file by file in path order.
pub(crate) struct Table<'a, 's> {
    pub(crate) own: Vec<&'a Decl<'s>>,
    pub(crate) instructions: Vec<Instruction<'s>>,
}

impl<'a, 's> Table<'a, 's> {
    /// The architecture `arch` among `sources`, all of `lib/lang/lang/arch`; `fill` reads a hole
    /// of a family's form as its member gives it, `None` for a value that is not a hole.
    pub(crate) fn read(
        generator: &Generator,
        arch: &str,
        sources: &'a [Source<'s>],
        fill: fn(&Literal<'s>, &Literal<'s>) -> Option<Literal<'s>>,
    ) -> Read<Self> {
        let unread = |what: &'static str| generator.error(UNREAD, arch, Msg::new().text(what));
        let own_path = format!("{ARCH}/{arch}.vun");
        let declaring =
            sources.iter().find(|s| s.path == own_path).ok_or_else(|| unread("has no declaration file"))?;
        let dir = format!("{ARCH}/{arch}/");
        let children = sources.iter().filter(|s| s.path.starts_with(&dir));
        // A declaration of a child file that is none of the table's values nor a shorthand is a type.
        let types = children.clone().flat_map(|s| &s.decls).filter(|d| {
            let table =
                |ty: &[&str]| matches!(ty.last().copied(), Some(INSTRUCTION | FAMILY | MEMBER | MNEMONIC));
            !d.name.starts_with(PRIVATE) && !matches!(&d.value, Literal::Typed(ty, _) if table(ty))
        });
        let own: Vec<&'a Decl<'s>> = declaring.decls.iter().chain(types).collect();
        let defaults = |ty: &str| {
            let decl = own.iter().find(|d| d.name == ty);
            let fields = decl.and_then(|d| d.value.members()).unwrap_or_default();
            fields.iter().filter(|f| f.value != Literal::Opaque).cloned().collect::<Vec<_>>()
        };
        let (form_defaults, member_defaults) = (defaults(FORM), defaults(MEMBER));
        let nested: Vec<_> = NESTED.iter().map(|&(field, ty)| (field, defaults(ty))).collect();
        let (mut instructions, mut spelled) = (Vec::new(), BTreeMap::new());
        for source in children {
            let shorthands: Fields<'s> = source
                .decls
                .iter()
                .filter(|d| d.name.starts_with(PRIVATE))
                .map(|d| (d.name, d.value.clone()))
                .collect();
            let resolve = |value: &Literal<'s>| resolved(value, &shorthands);
            // Each family's forms, and its doc lines, which its members share.
            let mut families: BTreeMap<&str, (Vec<Literal<'s>>, Vec<String>)> = BTreeMap::new();
            for decl in source.decls.iter().filter(|d| !d.name.starts_with(PRIVATE)) {
                let Literal::Typed(ty, record) = &decl.value else { continue };
                let forms = |record: &Literal<'s>| -> Read<Vec<Literal<'s>>> {
                    let forms = record.get(FORMS).map(resolve).and_then(|f| f.as_list().map(<[_]>::to_vec));
                    let forms = forms.ok_or_else(|| unread("declares an instruction without forms"))?;
                    Ok(forms
                        .iter()
                        .map(|f| with_nested(&with_defaults(f, &form_defaults), &nested))
                        .collect())
                };
                let (name, doc) = (decl.name, docs(&decl.doc));
                match ty.last().copied() {
                    Some(INSTRUCTION) => {
                        instructions.push(Instruction { name, mnemonic: None, doc, forms: forms(record)? });
                    }
                    Some(FAMILY) => {
                        families.insert(name, (forms(record)?, doc));
                    }
                    Some(MEMBER) => {
                        let member = with_defaults(&resolve(record), &member_defaults);
                        let family = member.get(OF_FAMILY).and_then(Literal::as_path);
                        let shared = family.and_then(|f| families.get(f.first()?));
                        let (shared, shared_doc) =
                            shared.ok_or_else(|| unread("declares a member of no family before it"))?;
                        let forms = shared.iter().map(|f| filled(f, &member, fill)).collect();
                        let doc = if doc.is_empty() { shared_doc.clone() } else { doc };
                        instructions.push(Instruction { name, mnemonic: None, doc, forms });
                    }
                    Some(MNEMONIC) => {
                        let spells = record.get(INSTRUCTIONS).and_then(Literal::as_list).unwrap_or_default();
                        for instruction in spells.iter().filter_map(Literal::as_name) {
                            spelled.insert(instruction, name.to_owned());
                        }
                    }
                    _ => {}
                }
            }
        }
        for instruction in &mut instructions {
            instruction.mnemonic = spelled.get(instruction.name).cloned();
        }
        Ok(Self { own, instructions })
    }

    /// The declaration of the architecture named `name`.
    pub(crate) fn own(&self, name: &str) -> Option<&'a Decl<'s>> {
        self.own.iter().find(|d| d.name == name).copied()
    }
}

/// Values by name: a type's field defaults, or a file's shorthands.
type Fields<'s> = BTreeMap<&'s str, Literal<'s>>;

/// `value` with the shorthands it names replaced by their values, and typed records untyped.
fn resolved<'s>(value: &Literal<'s>, shorthands: &Fields<'s>) -> Literal<'s> {
    let each = |items: &[Literal<'s>]| items.iter().map(|v| resolved(v, shorthands)).collect();
    match value {
        Literal::Path(path) if path.len() == 1 && shorthands.contains_key(path[0]) => {
            resolved(&shorthands[path[0]], shorthands)
        }
        Literal::Typed(_, inner) => resolved(inner, shorthands),
        Literal::List(items) => Literal::List(each(items)),
        Literal::Tagged(tag, payload) => Literal::Tagged(tag, each(payload)),
        Literal::Record(fields) => Literal::Record(
            fields.iter().map(|f| Decl { value: resolved(&f.value, shorthands), ..f.clone() }).collect(),
        ),
        other => other.clone(),
    }
}

/// `record` with each field it lacks taken from `defaults`, a type's fields with their values.
fn with_defaults<'s>(record: &Literal<'s>, defaults: &[Decl<'s>]) -> Literal<'s> {
    let mut fields = record.decls().map(<[_]>::to_vec).unwrap_or_default();
    for default in defaults {
        if !fields.iter().any(|f| f.name == default.name) {
            fields.push(default.clone());
        }
    }
    Literal::Record(fields)
}

/// `form` with the records of its lists given their types' defaults: `nested` has each field's.
fn with_nested<'s>(form: &Literal<'s>, nested: &[(&str, Vec<Decl<'s>>)]) -> Literal<'s> {
    let fields = form.decls().unwrap_or_default().iter().map(|f| {
        let defaults = nested.iter().find(|(name, _)| *name == f.name).map(|(_, d)| d);
        let value = match (defaults, &f.value) {
            (Some(defaults), Literal::List(items)) => Literal::List(
                items
                    .iter()
                    .map(|i| if i.decls().is_some() { with_defaults(i, defaults) } else { i.clone() })
                    .collect(),
            ),
            _ => f.value.clone(),
        };
        Decl { value, ..f.clone() }
    });
    Literal::Record(fields.collect())
}

/// `form` with each hole filled as `member` gives it.
fn filled<'s>(
    form: &Literal<'s>,
    member: &Literal<'s>,
    fill: fn(&Literal<'s>, &Literal<'s>) -> Option<Literal<'s>>,
) -> Literal<'s> {
    if let Some(value) = fill(form, member) {
        return value;
    }
    let each = |items: &[Literal<'s>]| items.iter().map(|v| filled(v, member, fill)).collect();
    match form {
        Literal::List(items) => Literal::List(each(items)),
        Literal::Tagged(tag, payload) => Literal::Tagged(tag, each(payload)),
        Literal::Record(fields) => Literal::Record(
            fields.iter().map(|f| Decl { value: filled(&f.value, member, fill), ..f.clone() }).collect(),
        ),
        other => other.clone(),
    }
}
