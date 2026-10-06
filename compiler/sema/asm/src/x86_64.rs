//! x86-64 (`lang.arch.x86_64`): its forms' widths listed as they are (`adds(f64, ...)` selects the
//! one at 64 bits), a value held in a general register or an SSE one for a float, and memory as
//! one sum of a base, an index times a scale and bytes (`mem(rax, scaled(rcx, 8), 16)`); which
//! form takes which operands, `vun-isa-x86_64` says.

use vun_hir::tree::ExprId;
use vun_inline::{ClassId, Disp, Mem};
use vun_items::Item;
use vun_items::lang::{
    arch_x86_64 as x86, arch_x86_64_Class as class, arch_x86_64_Family as family, arch_x86_64_Flow as flow,
    arch_x86_64_Form as form, arch_x86_64_Instruction as instruction, arch_x86_64_Member as member,
    arch_x86_64_Size as size,
};

use super::check::Term;
use super::given::Addressing;
use super::need::Needs;
use super::parts::Widths;
use super::table::{Family, Flow, Shape, Table};

pub(super) const SHAPE: Shape = Shape {
    instruction: x86::Instruction,
    instruction_forms: instruction::forms,
    family: Some(Family { forms: family::forms, member: x86::Member, of: member::family }),
    mnemonic: None,
    form_flow: form::flow,
    needs: Needs { form: form::requires, sized: None },
    flows: &[(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: x86::Size,
    cond: Some(x86::Cond),
    class: x86::Class,
    registers: x86::Registers,
    choice: None,
    widths: Widths { of: form::sizes, size: None, types: None, encoding: None },
    bits: &BITS,
    floats: false,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The scales an index is multiplied by.
const SCALES: [i64; 4] = [1, 2, 4, 8];

/// The class of register a value is held in: SSE for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::sse } else { class::general })
}

/// Memory as one sum of its parts: a base, an index times a scale, bytes and a symbol.
pub(super) fn memory(parts: Vec<Term>) -> Option<Mem<ExprId, ExprId, ExprId>> {
    let (mut base, mut index, mut bytes, mut symbol) = (None, None, 0_i64, None);
    for part in parts {
        match part {
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
