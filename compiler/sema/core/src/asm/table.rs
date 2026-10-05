//! An architecture's table as its namespace declares it: its instructions, each a value of its
//! `Instruction` or a `Member` of a `Family`, read from its child namespaces in path order and in
//! declaration order in each, so that their positions are the ones the generated assemblers
//! number them by (`vun_inline::OpId`); and which of them each declaration a block calls names:
//! an instruction or a member its own, a `Mnemonic` those it spells.

use std::collections::BTreeMap;
use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::db::{Exports, Namespace, Package, PackageId};
use vun_inline::{ClassId, FormId, OpId, SizeId};
use vun_intern::Symbol;
use vun_items::Item;
use vun_query::{Cx, Query};
use vun_sema_ty::item::{DeclRef, Declared, Site};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use super::Arch;
use super::need::Needs;
use super::parts::Widths;
use crate::env::{Env, ItemSite};
use crate::member::Members as TypeMembers;
use crate::target::Members;

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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Op {
    pub name: Symbol,
    /// Each form's value, of the architecture's `Form`.
    pub forms: Vec<Value>,
}

/// The declarations of an architecture's families of instructions, which share their forms.
pub(super) struct Family {
    pub(super) forms: Item,
    pub(super) member: Item,
    pub(super) of: Item,
}

/// The declarations of an architecture its table is read through, and what it tells of its own.
pub(super) struct Shape {
    pub(super) instruction: Item,
    pub(super) instruction_forms: Item,
    /// Its families, where it has any.
    pub(super) family: Option<Family>,
    /// Its `Mnemonic` and the field naming the instructions one spells, where it has any.
    pub(super) mnemonic: Option<(Item, Item)>,
    pub(super) form_flow: Item,
    /// How its forms say what a processor needs to have them.
    pub(super) needs: Needs,
    pub(super) flows: &'static [(Item, Flow)],
    /// The enums of widths, conditions (where the instruction set has any) and register classes.
    pub(super) size: Item,
    pub(super) cond: Option<Item>,
    pub(super) class: Item,
    /// Its `Registers`, which an enum naming registers declares what it names with, and its
    /// `Choice`, the kinds of named choice, which an enum of choices declares its kind with.
    pub(super) registers: Item,
    pub(super) choice: Option<Item>,
    /// How its forms' widths are read, and which parts of a call select them.
    pub(super) widths: Widths,
    /// The bits of each width.
    pub(super) bits: &'static [(Item, u32)],
}

/// An architecture's table: its instructions in order, the names of its widths, conditions and
/// register classes in declaration order, and the instructions each declaration a block may call
/// names, in the order they are tried.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Table {
    pub ops: Vec<Op>,
    pub sizes: Vec<Symbol>,
    /// The bits of each width, in the order of `sizes`.
    pub bits: Vec<Option<u32>>,
    pub conds: Vec<Symbol>,
    pub classes: Vec<Symbol>,
    pub callables: BTreeMap<DeclRef, Vec<OpId>>,
}

impl Table {
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

    /// The form `form` of `op`.
    #[must_use]
    pub fn form(&self, op: OpId, form: FormId) -> Option<&Value> {
        self.ops.get(usize::from(op.0))?.forms.get(usize::from(form.0))
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
pub(super) fn position(names: &[Symbol], name: Symbol) -> Option<u8> {
    names.iter().position(|&n| n == name).and_then(|i| u8::try_from(i).ok())
}

/// The variants of the enum type `ty`, in declaration order.
pub(super) fn variants_of(env: &Env<'_, '_>, ty: Ty) -> Result<Vec<Symbol>, Unevaluated> {
    let members = env.cx.get::<TypeMembers>(&ty).ok_or(Unevaluated::Unsupported)?;
    Ok(members.variants.iter().map(|v| v.name).collect())
}

/// The type `item` declares.
pub(super) fn declared(env: &Env<'_, '_>, item: Item) -> Result<Ty, Unevaluated> {
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
        let file = vun_hir::db::FileRef { package: ns.package, path: Symbol::intern(path) };
        let env = Env::of(cx, &file).ok_or(Unevaluated::Absent)?;
        let shape = arch.shape();
        let sizes = variants_of(&env, declared(&env, shape.size)?)?;
        let bits = sizes
            .iter()
            .map(|&s| shape.bits.iter().find(|(i, _)| i.symbol() == s).map(|&(_, b)| b))
            .collect();
        let mut table = Table {
            sizes,
            bits,
            conds: shape.cond.map(|c| variants_of(&env, declared(&env, c)?)).transpose()?.unwrap_or_default(),
            classes: variants_of(&env, declared(&env, shape.class)?)?,
            ..Table::default()
        };
        let instruction = declared(&env, shape.instruction)?;
        let family = shape
            .family
            .as_ref()
            .map(|f| Ok::<_, Unevaluated>((declared(&env, f.member)?, f)))
            .transpose()?;
        let mnemonic = shape.mnemonic.map(|(m, _)| declared(&env, m)).transpose()?;
        let mut mnemonics = Vec::new();
        for child in package.children(&ns.path) {
            let child = Namespace { package: ns.package, path: [ns.path.as_slice(), &[child]].concat() };
            for &name in cx.get::<Exports>(&child).iter() {
                let value = env.member(Value::Namespace(child.clone()), name)?;
                let Value::Struct { ty, .. } = &value else { continue };
                let Some(Site::Decl(decl)) = cx.get::<Declared>(&(child.clone(), name)) else { continue };
                let items = |value: &Value, field| {
                    Members::of(value).ok().and_then(|m| m.get(field).ok()).and_then(|f| f.clone().items())
                };
                let forms = if *ty == instruction {
                    items(&value, shape.instruction_forms)
                } else if let Some((_, of)) = family.filter(|(member, _)| ty == member) {
                    let members = Members::of(&value).map_err(|_| Unevaluated::Unsupported)?;
                    let shared = members.get(of.of).map_err(|_| Unevaluated::Unsupported)?;
                    items(shared, of.forms)
                } else {
                    if Some(*ty) == mnemonic {
                        mnemonics.push((decl, value, child.clone()));
                    }
                    continue;
                };
                let op = OpId(u16::try_from(table.ops.len()).map_err(|_| Unevaluated::Unsupported)?);
                table.ops.push(Op { name, forms: forms.unwrap_or_default() });
                table.callables.insert(decl, vec![op]);
            }
        }
        // A mnemonic names the instructions it spells, each declared beside it.
        for (decl, value, child) in mnemonics {
            let Some((_, field)) = shape.mnemonic else { continue };
            let spelled = Members::of(&value).ok().and_then(|m| m.get(field).ok().cloned());
            let mut ops = Vec::new();
            for name in spelled.and_then(Value::items).unwrap_or_default() {
                let Value::Name(name) = name else { return Err(Unevaluated::Unsupported) };
                let Some(Site::Decl(of)) = cx.get::<Declared>(&(child.clone(), name)) else {
                    return Err(Unevaluated::Absent);
                };
                ops.extend(table.callables.get(&of).into_iter().flatten().copied());
            }
            table.callables.insert(decl, ops);
        }
        Ok(Arc::new(table))
    }
}
