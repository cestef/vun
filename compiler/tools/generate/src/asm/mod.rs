//! The assemblers' tables, from `lib/lang/lang/arch` (spec 07, section 6): each architecture's
//! instructions as declared there, families expanded, become its `Op` enum and the forms of each.
//!
//! An architecture `a` declares its types and registers in `arch/a.vun` and its instructions in
//! `arch/a/*.vun`. An instruction is a value of `Instruction`, or a member of a `Family`, whose
//! forms hold holes the member fills. A form's absent fields take the defaults `Form` declares, and
//! a name starting with `_` is a shorthand of its file, read as the value it names.

mod x86_64;

use std::collections::BTreeMap;

use vun_diag::{DiagId, Msg};
use vun_syntax::literal::{Decl, Literal};

use crate::generate::{Generator, Read, Source};
use crate::names::docs;

pub(crate) use x86_64::X86_64;

/// Where the architectures are declared, relative to the repository root.
const ARCH: &str = "lib/lang/lang/arch";

/// The declarations whose values are instructions and families, by the last segment of their
/// type's path, and the type whose field defaults absent fields take.
const INSTRUCTION: &str = "Instruction";
const FAMILY: &str = "Family";
const FORM: &str = "Form";
const MEMBER: &str = "Member";
/// The fields of an instruction and a family.
const FORMS: &str = "forms";
const MEMBERS: &str = "members";
const NAME: &str = "name";

/// A name that is a shorthand of its file rather than a declaration of the architecture.
const PRIVATE: char = '_';

const UNREAD: DiagId = DiagId::new("generate.asm.unread");

/// An instruction: its name, its doc lines, and its forms, each field present.
pub(crate) struct Instruction<'s> {
    pub(crate) name: &'s str,
    pub(crate) doc: Vec<String>,
    pub(crate) forms: Vec<Literal<'s>>,
}

/// An architecture as its files declare it: its own declarations, and its instructions, file by
/// file in path order.
pub(crate) struct Table<'a, 's> {
    pub(crate) own: &'a [Decl<'s>],
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
        let own =
            sources.iter().find(|s| s.path == own_path).ok_or_else(|| unread("has no declaration file"))?;
        let defaults = |ty: &str| {
            let decl = own.decls.iter().find(|d| d.name == ty);
            let fields = decl.and_then(|d| d.value.members()).unwrap_or_default();
            fields.iter().filter(|f| f.value != Literal::Opaque).cloned().collect::<Vec<_>>()
        };
        let (form_defaults, member_defaults) = (defaults(FORM), defaults(MEMBER));
        let dir = format!("{ARCH}/{arch}/");
        let mut instructions = Vec::new();
        for source in sources.iter().filter(|s| s.path.starts_with(&dir)) {
            let shorthands: Fields<'s> = source
                .decls
                .iter()
                .filter(|d| d.name.starts_with(PRIVATE))
                .map(|d| (d.name, d.value.clone()))
                .collect();
            let resolve = |value: &Literal<'s>| resolved(value, &shorthands);
            for decl in source.decls.iter().filter(|d| !d.name.starts_with(PRIVATE)) {
                let Literal::Typed(ty, record) = &decl.value else { continue };
                let forms = |record: &Literal<'s>| -> Read<Vec<Literal<'s>>> {
                    let forms = record.get(FORMS).map(resolve).and_then(|f| f.as_list().map(<[_]>::to_vec));
                    let forms = forms.ok_or_else(|| unread("declares an instruction without forms"))?;
                    Ok(forms.iter().map(|f| with_defaults(f, &form_defaults)).collect())
                };
                match ty.last().copied() {
                    Some(INSTRUCTION) => {
                        instructions.push(Instruction {
                            name: decl.name,
                            doc: docs(&decl.doc),
                            forms: forms(record)?,
                        });
                    }
                    Some(FAMILY) => {
                        let shared = forms(record)?;
                        let members = record.get(MEMBERS).map(resolve);
                        for member in members.as_ref().and_then(Literal::as_list).unwrap_or_default() {
                            let member = with_defaults(&resolve(member), &member_defaults);
                            let name = member.get(NAME).and_then(Literal::as_name);
                            let name = name.ok_or_else(|| unread("declares a member without a name"))?;
                            let forms = shared.iter().map(|f| filled(f, &member, fill)).collect();
                            instructions.push(Instruction { name, doc: docs(&decl.doc), forms });
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(Self { own: &own.decls, instructions })
    }

    /// The declaration of the architecture named `name`.
    pub(crate) fn own(&self, name: &str) -> Option<&'a Decl<'s>> {
        self.own.iter().find(|d| d.name == name)
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
