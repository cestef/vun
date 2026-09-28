//! x86-64 (`lang.arch.x86_64`): registers named by width as `names` gives them and SSE registers
//! by their encoding, conditions as a suffix of the mnemonic (`jz`), and memory as one sum
//! `[base + index * scale + disp]`; which form takes which operands, `vun-isa-x86_64` says.

use vun_hir::tree::ExprId;
use vun_inline::{ClassId, Disp, Mem, RegRef};
use vun_intern::Symbol;
use vun_isa::Fixed;
use vun_items::Item;
use vun_items::lang::{
    arch_x86_64 as x86, arch_x86_64_Class as class, arch_x86_64_Family as family, arch_x86_64_Flow as flow,
    arch_x86_64_Form as form, arch_x86_64_Instruction as instruction, arch_x86_64_Kind as kind,
    arch_x86_64_Member as member, arch_x86_64_Operand as operand, arch_x86_64_Size as size,
};

use super::check::Term;
use super::given::{Addressing, Word};
use super::spell::{At, Mnemonic, Spelled};
use super::table::{Flow, Shape, Table, variants};
use crate::env::{Env, Unevaluated};
use crate::target::Members;
use crate::ty::Value;

pub(super) const SHAPE: Shape = Shape {
    instruction: x86::Instruction,
    instruction_forms: instruction::forms,
    instruction_mnemonic: None,
    family: x86::Family,
    family_forms: family::forms,
    members: family::members,
    member_name: member::name,
    form_name: form::name,
    form_flow: form::flow,
    flows: [(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: x86::Size,
    cond: x86::Cond,
    class: x86::Class,
    registers,
    spellings,
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The scales an index is multiplied by.
const SCALES: [i64; 4] = [1, 2, 4, 8];

/// General registers by each width's name, and SSE registers by their encoding's.
fn registers(env: &Env<'_, '_>, table: &mut Table) -> Result<(), Unevaluated> {
    let (general, sse) = (table.class(class::general), table.class(class::sse));
    let (Some(general), Some(sse)) = (general, sse) else { return Err(Unevaluated::Unsupported) };
    let encodings = variants(env, x86::Reg)?;
    let Value::Struct { fields, .. } = env.item(x86::names)? else { return Err(Unevaluated::Unsupported) };
    for (name, names) in &fields {
        let number = encodings.iter().position(|e| e == name).ok_or(Unevaluated::Unsupported)?;
        let reg =
            RegRef { class: general, number: u8::try_from(number).map_err(|_| Unevaluated::Unsupported)? };
        let Value::Struct { fields: widths, .. } = names else { return Err(Unevaluated::Unsupported) };
        for (width, text) in widths {
            let text = text.text().ok_or(Unevaluated::Unsupported)?;
            table.words.insert(Symbol::intern(&text), Word::Reg { reg, size: table.size(*width) });
        }
    }
    for (number, name) in variants(env, x86::Xmm)?.into_iter().enumerate() {
        let reg = RegRef { class: sse, number: u8::try_from(number).map_err(|_| Unevaluated::Unsupported)? };
        table.words.insert(name, Word::Reg { reg, size: None });
    }
    Ok(())
}

/// The kind of each operand of `form`.
fn kinds(form: &Value) -> Vec<Value> {
    let operands = Members::of(form).ok().and_then(|m| m.get(form::operands).ok().cloned());
    let operands = operands.and_then(Value::items).unwrap_or_default();
    operands.iter().filter_map(|o| Members::of(o).ok()?.get(operand::kind).ok().cloned()).collect()
}

/// Whether `kind` is the variant `item`.
fn is(kind: &Value, item: Item) -> bool {
    matches!(kind, Value::Variant { name, .. } if *name == item.symbol())
}

/// A form taking a condition is spelled with each condition as a suffix: `j` as `jz`, `jnz`, ...
fn spellings(table: &Table, name: Symbol, form: &Value) -> Vec<(Mnemonic, Fixed)> {
    let spelled = Spelled::of(name);
    if kinds(form).iter().any(|k| is(k, kind::cond)) { spelled.conds(table, At::Glued) } else { spelled }
        .into_vec()
}

/// The class of register a value is held in: SSE for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::sse } else { class::general })
}

/// Memory as one sum of its terms: a base, an index times a scale, bytes and a symbol.
pub(super) fn memory(terms: Vec<Term>) -> Option<Mem<ExprId, ExprId, ExprId>> {
    let (mut base, mut index, mut bytes, mut symbol) = (None, None, 0_i64, None);
    for term in terms {
        match term {
            Term::Reg(given) if base.is_none() => base = Some(given.base()?),
            Term::Reg(given) if index.is_none() => index = Some((given.base()?, 1)),
            Term::Scaled(given, scale) if index.is_none() && SCALES.contains(&scale) => {
                index = Some((given.base()?, u8::try_from(scale).ok()?));
            }
            Term::Bytes(n) => bytes = bytes.checked_add(n)?,
            Term::Symbol(s) if symbol.is_none() => symbol = Some(s),
            _ => return None,
        }
    }
    let disp = symbol.map_or(Disp::Bytes(bytes), |s| Disp::Symbol(s, bytes));
    Some(Mem { base, index, disp })
}
