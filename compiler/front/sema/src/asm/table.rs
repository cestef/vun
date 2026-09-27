//! An architecture's table as its namespace declares it: its instructions, each a value of its
//! `Instruction` or a member of a `Family`, read from its child namespaces in path order and in
//! declaration order in each, families expanded, so that their positions are the ones the
//! generated assemblers number them by (`vun_inline::OpId`, `FormId`).

use std::collections::BTreeMap;
use std::sync::Arc;

use vun_hir::db::{Exports, FileRef, Namespace, Package, PackageId};
use vun_inline::{ClassId, CondId, FormId, OpId, RegRef, SizeId};
use vun_intern::Symbol;
use vun_items::Item;
use vun_query::{Cx, Query};

use super::Arch;
use super::given::Word;
use crate::env::{Env, ItemSite, Unevaluated};
use crate::item::Site;
use crate::member::Members as TypeMembers;
use crate::target::Members;
use crate::ty::{Ty, Value};

/// Where control goes after a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flow {
    /// To the next instruction, and for a conditional branch also to its target.
    Next,
    /// To its target only.
    Jump,
    /// Out of the code: it returns or traps.
    Ends,
}

/// What a spelling of an instruction fixes beyond its forms: a condition written as a suffix of
/// the mnemonic (`jz`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fixed {
    Nothing,
    Cond(CondId),
}

/// A form an instruction is spelled with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spelling {
    pub op: OpId,
    pub form: FormId,
    pub fixed: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Op {
    pub name: Symbol,
    /// Each form's value, of the architecture's `Form`.
    pub forms: Vec<Value>,
}

/// The declarations of an architecture its table is read through, and what it tells of its own.
pub(super) struct Shape {
    pub(super) instruction: Item,
    pub(super) instruction_forms: Item,
    pub(super) family: Item,
    pub(super) family_forms: Item,
    pub(super) members: Item,
    pub(super) member_name: Item,
    /// The field of a form naming its mnemonic, where it is not its instruction's.
    pub(super) form_name: Item,
    pub(super) form_flow: Item,
    pub(super) flows: [(Item, Flow); 3],
    /// The enums of widths, conditions and register classes.
    pub(super) size: Item,
    pub(super) cond: Item,
    pub(super) class: Item,
    /// Reads the architecture's registers into the table's `words` and `registers`.
    pub(super) registers: fn(&Env<'_, '_>, &mut Table) -> Result<(), Unevaluated>,
    /// The mnemonics a form of an instruction is spelled with, and what each fixes.
    pub(super) spellings: fn(&Table, Symbol, &Value) -> Vec<(Mnemonic, Fixed)>,
}

/// A mnemonic as a line writes it: a word, and the suffix after a `.` (`b.eq`).
pub type Mnemonic = (Symbol, Option<Symbol>);

/// An architecture's table: its instructions in order, the names of its widths, conditions and
/// register classes in declaration order, its registers by each name a line writes and by the
/// name of their encoding, and the forms each mnemonic spells.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Table {
    pub ops: Vec<Op>,
    pub sizes: Vec<Symbol>,
    pub conds: Vec<Symbol>,
    pub classes: Vec<Symbol>,
    pub words: BTreeMap<Symbol, Word>,
    pub registers: BTreeMap<Symbol, RegRef>,
    pub spellings: BTreeMap<Mnemonic, Vec<Spelling>>,
}

impl Table {
    /// The form `spelling` names.
    #[must_use]
    pub fn form(&self, spelling: Spelling) -> &Value {
        &self.ops[usize::from(spelling.op.0)].forms[usize::from(spelling.form.0)]
    }

    /// The width `item` names, a variant of the architecture's `Size`.
    #[must_use]
    pub fn size(&self, item: Symbol) -> Option<SizeId> {
        position(&self.sizes, item).map(SizeId)
    }

    /// The register class `item` names, a variant of the architecture's `Class`.
    #[must_use]
    pub fn class(&self, item: Item) -> Option<ClassId> {
        position(&self.classes, item.symbol()).map(ClassId)
    }

    /// Where control goes after `form`, of `arch`.
    #[must_use]
    pub fn flow(arch: Arch, form: &Value) -> Flow {
        let shape = arch.shape();
        let variant = Members::of(form).ok().and_then(|m| m.get(shape.form_flow).ok().cloned());
        let Some(Value::Variant { name, .. }) = variant else { return Flow::Next };
        shape.flows.iter().find(|(item, _)| item.symbol() == name).map_or(Flow::Next, |&(_, f)| f)
    }
}

/// Where `name` is among `names`, as the index of a table.
fn position(names: &[Symbol], name: Symbol) -> Option<u8> {
    names.iter().position(|&n| n == name).and_then(|i| u8::try_from(i).ok())
}

/// The variants of the enum `item` declares, in declaration order.
pub(super) fn variants(env: &Env<'_, '_>, item: Item) -> Result<Vec<Symbol>, Unevaluated> {
    let Value::Type(ty) = env.item(item)? else { return Err(Unevaluated::Unsupported) };
    let members = env.cx.get::<TypeMembers>(&ty).ok_or(Unevaluated::Unsupported)?;
    Ok(members.variants.iter().map(|v| v.name).collect())
}

/// The type `item` declares.
fn declared(env: &Env<'_, '_>, item: Item) -> Result<Ty, Unevaluated> {
    match env.item(item)? {
        Value::Type(ty) => Ok(ty),
        _ => Err(Unevaluated::Unsupported),
    }
}

/// The table of an architecture, as the files of a package see its namespace in `lang.arch`.
pub struct ArchTable;

impl Query for ArchTable {
    const NAME: &'static str = "arch_table";
    type Key = (PackageId, Arch);
    type Value = Result<Arc<Table>, Unevaluated>;

    fn compute(cx: &Cx<'_>, (seen, arch): &Self::Key) -> Self::Value {
        let Site::Namespace(ns) = cx.get::<ItemSite>(&(*seen, arch.namespace()))? else {
            return Err(Unevaluated::Unsupported);
        };
        let package = cx.get::<Package>(&ns.package);
        let path = package.file_of(&ns.path).ok_or(Unevaluated::Absent)?;
        let file = FileRef { package: ns.package, path: Symbol::intern(path) };
        let env = Env::of(cx, &file).ok_or(Unevaluated::Absent)?;
        let shape = arch.shape();
        let mut table = Table {
            sizes: variants(&env, shape.size)?,
            conds: variants(&env, shape.cond)?,
            classes: variants(&env, shape.class)?,
            ..Table::default()
        };
        let (instruction, family) = (declared(&env, shape.instruction)?, declared(&env, shape.family)?);
        for child in package.children(&ns.path) {
            let child = Namespace { package: ns.package, path: [ns.path.as_slice(), &[child]].concat() };
            for &name in cx.get::<Exports>(&child).iter() {
                let value = env.member(Value::Namespace(child.clone()), name)?;
                let Value::Struct { ty, .. } = &value else { continue };
                let members = Members::of(&value).map_err(|_| Unevaluated::Unsupported)?;
                let forms = |field| members.get(field).ok().and_then(|f| f.clone().items());
                if *ty == instruction {
                    table.ops.push(Op { name, forms: forms(shape.instruction_forms).unwrap_or_default() });
                } else if *ty == family {
                    let shared = forms(shape.family_forms).unwrap_or_default();
                    let family = members.get(shape.members).ok().and_then(|m| m.clone().items());
                    for member in family.unwrap_or_default() {
                        let name = Members::of(&member).and_then(|m| m.name(shape.member_name));
                        let name = name.map_err(|_| Unevaluated::Unsupported)?;
                        table.ops.push(Op { name, forms: shared.clone() });
                    }
                }
            }
        }
        (shape.registers)(&env, &mut table)?;
        table.spellings = spellings(&table, shape);
        Ok(Arc::new(table))
    }
}

/// The forms each mnemonic spells, in the table's order: the first that fits is chosen.
fn spellings(table: &Table, shape: &Shape) -> BTreeMap<Mnemonic, Vec<Spelling>> {
    let mut spellings: BTreeMap<Mnemonic, Vec<Spelling>> = BTreeMap::new();
    for (op, instruction) in table.ops.iter().enumerate() {
        for (form, value) in instruction.forms.iter().enumerate() {
            let (Ok(op), Ok(form)) = (u16::try_from(op), u16::try_from(form)) else { continue };
            let named = Members::of(value).ok().and_then(|m| m.symbol(shape.form_name).ok().flatten());
            let name = named.unwrap_or(instruction.name);
            for (mnemonic, fixed) in (shape.spellings)(table, name, value) {
                spellings.entry(mnemonic).or_default().push(Spelling {
                    op: OpId(op),
                    form: FormId(form),
                    fixed,
                });
            }
        }
    }
    spellings
}
