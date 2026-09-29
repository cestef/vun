//! Thumb (`lang.arch.thumb`): general and special registers named as their encodings are (`r0`,
//! `primask`), floating-point ones as `fp_names` names them at each width (`s0`, `d0`), interrupt
//! masks and barrier domains as words (`i`, `sy`), a condition glued to the mnemonic (`beq`), a
//! form's encoding width and its types each after a `.` (`add.w`, `vcvt.s32.f32`), and memory as
//! `[base]`, `[base, offset]` or `[base, index]`; which form takes which operands,
//! `vun-isa-thumb` says.

use vun_inline::{ClassId, RegRef};
use vun_intern::Symbol;
use vun_isa::{ChoiceId, Fixed};
use vun_items::Item;
use vun_items::lang::{
    arch_thumb as t, arch_thumb_Arg as arg, arch_thumb_Choice as choice, arch_thumb_Class as class,
    arch_thumb_Encoding as encoding, arch_thumb_Family as family, arch_thumb_Flow as flow,
    arch_thumb_Form as form, arch_thumb_Instruction as instruction, arch_thumb_Member as member,
    arch_thumb_Need as need_of, arch_thumb_Size as size, arch_thumb_Sized as sized,
    arch_thumb_operand_Kind as kind,
};

use super::given::Word;
use super::need::{self, Needs};
use super::spell::{At, Mnemonic, Sizes, Spelled, Width};
use super::table::{Flow, Shape, Table, variants};
use crate::env::{Env, Unevaluated};
use crate::target::Members;
use crate::ty::Value;

pub(super) const SHAPE: Shape = Shape {
    instruction: t::Instruction,
    instruction_forms: instruction::forms,
    instruction_mnemonic: Some(instruction::mnemonic),
    family: t::Family,
    family_forms: family::forms,
    members: family::members,
    member_name: member::name,
    form_name: form::name,
    form_flow: form::flow,
    needs: Some(Needs {
        form: form::requires,
        sized: Some(need::Sized { sizes: form::sizes, size: sized::size, requires: sized::requires }),
        feature: need_of::feature,
        float: Some(need_of::float),
    }),
    flows: [(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: t::Size,
    cond: t::Cond,
    class: t::Class,
    registers,
    spellings,
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The enum of the choices of each kind of `Choice`.
const CHOICES: [(Item, Item); 2] = [(choice::mask, t::Mask), (choice::barrier, t::Barrier)];

/// The registers named as their encodings are, each enum with its class.
const NAMED: [(Item, Item); 3] =
    [(t::Reg, class::general), (t::Special, class::special), (t::Status, class::status)];

/// How a form's widths are read: each with its types' qualifiers (`vadd.f32`).
const SIZES: Sizes =
    Sizes { of: form::sizes, size: sized::size, suffix: None, qualifiers: Some(sized::qualifiers) };

/// A number of the table, as an index of its enum.
fn number(i: usize) -> Result<u8, Unevaluated> {
    u8::try_from(i).map_err(|_| Unevaluated::Unsupported)
}

/// General registers at 32 bits and the others by their encodings' names, floating-point ones by
/// each width's name, and the choices of each kind by theirs.
fn registers(env: &Env<'_, '_>, table: &mut Table) -> Result<(), Unevaluated> {
    let word = table.size(size::b32.symbol());
    for (encodings, class) in NAMED {
        let class = table.class(class).ok_or(Unevaluated::Unsupported)?;
        let size =
            if class == table.class(class::general).ok_or(Unevaluated::Unsupported)? { word } else { None };
        for (at, name) in variants(env, encodings)?.into_iter().enumerate() {
            table.words.insert(name, Word::Reg { reg: RegRef { class, number: number(at)? }, size });
        }
    }
    let fp = table.class(class::fp).ok_or(Unevaluated::Unsupported)?;
    let encodings = variants(env, t::V)?;
    let Value::Struct { fields: widths, .. } = env.item(t::fp_names)? else {
        return Err(Unevaluated::Unsupported);
    };
    for (width, names) in &widths {
        let Value::Struct { fields: names, .. } = names else { return Err(Unevaluated::Unsupported) };
        for (v, text) in names {
            let at = encodings.iter().position(|e| e == v).ok_or(Unevaluated::Unsupported)?;
            let text = text.text().ok_or(Unevaluated::Unsupported)?;
            let reg = RegRef { class: fp, number: number(at)? };
            table.words.insert(Symbol::intern(&text), Word::Reg { reg, size: table.size(*width) });
        }
    }
    let kinds = variants(env, t::Choice)?;
    for (kind, choices) in CHOICES {
        let at = kinds.iter().position(|&k| k == kind.symbol()).ok_or(Unevaluated::Unsupported)?;
        let of = ChoiceId(number(at)?);
        for (i, name) in variants(env, choices)?.into_iter().enumerate() {
            table.words.insert(name, Word::Choice { of, index: number(i)? });
        }
    }
    Ok(())
}

/// The width of a form's encoding: a base that is a halfword is a 16-bit form.
fn encoding(form: &Value) -> Option<Item> {
    let Some(Value::Int(base)) = Members::of(form).ok()?.get(form::base).ok().cloned() else { return None };
    Some(if u16::try_from(base.to_u128()?).is_ok() { encoding::n } else { encoding::w })
}

/// A form is spelled as its mnemonic, with each condition glued where it takes one (`beq`), as it
/// is or with its encoding's width after a `.` (`add.w`), then its types (`vadd.f32`).
fn spellings(table: &Table, name: Symbol, form: &Value) -> Vec<(Mnemonic, Fixed)> {
    let field = |value: &Value, item: Item| Members::of(value).ok()?.get(item).ok().cloned();
    let args = field(form, form::args).and_then(Value::items).unwrap_or_default();
    let conditional = args
        .iter()
        .filter_map(|a| field(a, arg::kind))
        .any(|k| matches!(k, Value::Variant { name, .. } if name == kind::cond.symbol()));
    let mut spelled = Spelled::of(name);
    if conditional {
        spelled = spelled.conds(table, At::Glued);
    }
    if let Some(width) = encoding(form) {
        spelled = spelled.qualified(width.symbol());
    }
    spelled.sized(&Width::of(table, form, &SIZES)).into_vec()
}

/// The class of register a value is held in: floating-point for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::fp } else { class::general })
}
