//! Thumb (`lang.arch.thumb`): a form's widths named by the types it computes (`vcvt(i32, f32,
//! ...)` is `vcvt.s32.f32`), its encoding's width, which a call may write (`add(w, ...)` is
//! `add.w`), a value held in a general register or a floating-point one for a float, and memory as
//! the Arm architectures write it (`bracket`); which form takes which operands, `vun-isa-thumb`
//! says.

use vun_inline::ClassId;
use vun_items::Item;
use vun_items::lang::{
    arch_thumb as t, arch_thumb_Class as class, arch_thumb_Encoding as encoding, arch_thumb_Family as family,
    arch_thumb_Flow as flow, arch_thumb_Form as form, arch_thumb_Instruction as instruction,
    arch_thumb_Member as member, arch_thumb_Mnemonic as mnemonic, arch_thumb_Size as size,
    arch_thumb_Sized as sized,
};
use vun_sema_ty::Value;

use super::need::{self, Needs};
use super::parts::Widths;
use super::table::{Flow, Shape, Table};
use crate::target::Members;

pub(super) const SHAPE: Shape = Shape {
    instruction: t::Instruction,
    instruction_forms: instruction::forms,
    family_forms: family::forms,
    member: t::Member,
    member_family: member::family,
    mnemonic: Some((t::Mnemonic, mnemonic::instructions)),
    form_flow: form::flow,
    needs: Needs {
        form: form::requires,
        sized: Some(need::Sized { sizes: form::sizes, size: sized::size, requires: sized::requires }),
    },
    flows: [(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: t::Size,
    cond: t::Cond,
    class: t::Class,
    registers: t::Registers,
    choice: Some(t::Choice),
    widths: Widths {
        of: form::sizes,
        size: Some(sized::size),
        types: Some(sized::types),
        encoding: Some((t::Encoding, encoded)),
    },
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The width of a form's encoding: a base that is a halfword is a 16-bit form.
fn encoded(form: &Value) -> Option<Item> {
    let Some(Value::Int(base)) = Members::of(form).ok()?.get(form::base).ok().cloned() else { return None };
    Some(if u16::try_from(base.to_u128()?).is_ok() { encoding::n } else { encoding::w })
}

/// The class of register a value is held in: floating-point for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::fp } else { class::general })
}
