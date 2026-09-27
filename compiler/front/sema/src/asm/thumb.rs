//! Thumb (`lang.arch.thumb`): general and special registers named as their encodings are (`r0`,
//! `primask`), interrupt masks and barrier domains as words (`i`, `sy`), each mnemonic spelled as
//! its instruction is, and no memory operand; which form takes which operands,
//! `vun-isa-thumb` says.

use vun_inline::{ClassId, RegRef};
use vun_intern::Symbol;
use vun_isa::{ChoiceId, Fixed};
use vun_items::Item;
use vun_items::lang::{
    arch_thumb as t, arch_thumb_Choice as choice, arch_thumb_Class as class, arch_thumb_Family as family,
    arch_thumb_Flow as flow, arch_thumb_Form as form, arch_thumb_Instruction as instruction,
    arch_thumb_Member as member, arch_thumb_Size as size,
};

use super::given::Word;
use super::table::{Flow, Mnemonic, Shape, Table, variants};
use crate::env::{Env, Unevaluated};
use crate::ty::Value;

pub(super) const SHAPE: Shape = Shape {
    instruction: t::Instruction,
    instruction_forms: instruction::forms,
    instruction_mnemonic: None,
    family: t::Family,
    family_forms: family::forms,
    members: family::members,
    member_name: member::name,
    form_name: form::name,
    form_flow: form::flow,
    flows: [(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: t::Size,
    cond: t::Cond,
    class: t::Class,
    registers,
    spellings,
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 1] = [(size::b32, 32)];

/// The enum of the choices of each kind of `Choice`.
const CHOICES: [(Item, Item); 2] = [(choice::mask, t::Mask), (choice::barrier, t::Barrier)];

/// A number of the table, as an index of its enum.
fn number(i: usize) -> Result<u8, Unevaluated> {
    u8::try_from(i).map_err(|_| Unevaluated::Unsupported)
}

/// General registers at 32 bits and special ones by their encodings' names, and the choices of
/// each kind by theirs.
fn registers(env: &Env<'_, '_>, table: &mut Table) -> Result<(), Unevaluated> {
    let (Some(general), Some(special)) = (table.class(class::general), table.class(class::special)) else {
        return Err(Unevaluated::Unsupported);
    };
    let width = table.size(size::b32.symbol());
    for (encodings, class, size) in [(t::Reg, general, width), (t::Special, special, None)] {
        for (at, name) in variants(env, encodings)?.into_iter().enumerate() {
            let reg = RegRef { class, number: number(at)? };
            table.words.insert(name, Word::Reg { reg, size });
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

/// Each form is spelled as its mnemonic alone.
fn spellings(_: &Table, name: Symbol, _: &Value) -> Vec<(Mnemonic, Fixed)> {
    vec![((name, None), Fixed::default())]
}

/// The class of register a value is held in: a general one, since the table has no
/// floating-point registers; `None` for a float.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    if float { None } else { table.class(class::general) }
}
