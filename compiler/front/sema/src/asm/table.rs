//! An architecture's table as its namespace declares it: its instructions, each a value of its
//! `Instruction` or a member of a `Family`, read from its child namespaces in path order and in
//! declaration order in each, families expanded, so that their positions are the ones the
//! generated assemblers number them by (`vun_inline::OpId`, `FormId`).

use std::collections::BTreeMap;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::db::{Exports, FileRef, Namespace, Package, PackageId};
use vun_inline::{ClassId, FormId, OpId, SizeId};
use vun_intern::Symbol;
use vun_items::Item;
use vun_query::{Cx, Query};

use super::Arch;
use super::given::Word;
use super::need::Needs;
use super::spell::Mnemonic;
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

/// A form an instruction is spelled with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Spelling {
    pub op: OpId,
    pub form: FormId,
    pub fixed: vun_isa::Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Op {
    pub name: Symbol,
    /// Each form's value, of the architecture's `Form`.
    pub forms: Vec<Value>,
}

/// The declarations of an architecture its table is read through, and what it tells of its own.
pub(super) struct Shape {
    pub(super) instruction: Item,
    pub(super) instruction_forms: Item,
    /// The field of an instruction naming its mnemonic, where it is not its name.
    pub(super) instruction_mnemonic: Option<Item>,
    pub(super) family: Item,
    pub(super) family_forms: Item,
    pub(super) members: Item,
    pub(super) member_name: Item,
    /// The field of a form naming its mnemonic, where it is not its instruction's.
    pub(super) form_name: Item,
    pub(super) form_flow: Item,
    /// How its forms say what a processor needs to have them, where they do.
    pub(super) needs: Option<Needs>,
    pub(super) flows: [(Item, Flow); 3],
    /// The enums of widths, conditions and register classes.
    pub(super) size: Item,
    pub(super) cond: Item,
    pub(super) class: Item,
    /// Reads the architecture's words into the table.
    pub(super) registers: fn(&Env<'_, '_>, &mut Table) -> Result<(), Unevaluated>,
    /// The mnemonics a form of an instruction is spelled with, and what each fixes.
    pub(super) spellings: fn(&Table, Symbol, &Value) -> Vec<(Mnemonic, vun_isa::Fixed)>,
    /// The bits of each width.
    pub(super) bits: &'static [(Item, u32)],
}

/// An architecture's table: its instructions in order, the names of its widths, conditions and
/// register classes in declaration order, its words by the name a line writes, and the forms each
/// mnemonic spells.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Table {
    pub ops: Vec<Op>,
    pub sizes: Vec<Symbol>,
    /// The bits of each width, in the order of `sizes`.
    pub bits: Vec<Option<u32>>,
    pub conds: Vec<Symbol>,
    pub classes: Vec<Symbol>,
    pub words: BTreeMap<Symbol, Word>,
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

    /// The width of a value of `bytes` bytes held in a register.
    #[must_use]
    pub fn width(&self, bytes: u64) -> Option<SizeId> {
        let bits = u32::try_from(bytes.checked_mul(8)?).ok()?;
        self.bits.iter().position(|&b| b == Some(bits)).and_then(|i| u8::try_from(i).ok()).map(SizeId)
    }

    /// The bits of the width `size`.
    #[must_use]
    pub fn bits(&self, size: SizeId) -> Option<u32> {
        *self.bits.get(usize::from(size.0))?
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
        let sizes = variants(&env, shape.size)?;
        let bits = sizes
            .iter()
            .map(|&s| shape.bits.iter().find(|(i, _)| i.symbol() == s).map(|&(_, b)| b))
            .collect();
        let mut table = Table {
            sizes,
            bits,
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
                    let named = shape.instruction_mnemonic.and_then(|m| members.symbol(m).ok().flatten());
                    let forms = forms(shape.instruction_forms).unwrap_or_default();
                    table.ops.push(Op { name: named.unwrap_or(name), forms });
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
