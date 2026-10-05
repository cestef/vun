//! 64-bit ARM (`lang.arch.arm64`): an access width named by the mnemonic's suffix, which a call
//! writes as the type it accesses (`ldr(u8, ...)` is `ldrb`), a value held in a general register or
//! a floating-point one for a float, and memory as the Arm architectures write it (`bracket`);
//! which form takes which operands, `vun-isa-arm64` says.

use vun_inline::ClassId;
use vun_items::Item;
use vun_items::lang::{
    arch_arm64 as a, arch_arm64_Class as class, arch_arm64_Family as family, arch_arm64_Flow as flow,
    arch_arm64_Form as form, arch_arm64_Instruction as instruction, arch_arm64_Member as member,
    arch_arm64_Mnemonic as mnemonic, arch_arm64_Size as size, arch_arm64_Sized as sized,
};

use super::need::Needs;
use super::parts::Widths;
use super::table::{Family, Flow, Shape, Table};

pub(super) const SHAPE: Shape = Shape {
    instruction: a::Instruction,
    instruction_forms: instruction::forms,
    family: Some(Family { forms: family::forms, member: a::Member, of: member::family }),
    mnemonic: Some((a::Mnemonic, mnemonic::instructions)),
    form_flow: form::flow,
    needs: Needs { form: form::requires, sized: None },
    flows: &[(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: a::Size,
    cond: Some(a::Cond),
    class: a::Class,
    registers: a::Registers,
    choice: Some(a::Choice),
    widths: Widths { of: form::sizes, size: Some(sized::size), types: Some(sized::types), encoding: None },
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The class of register a value is held in: floating-point for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::fp } else { class::general })
}
