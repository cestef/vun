//! WebAssembly (`lang.arch.wasm32`): a value held in a local of the type its size and float-ness
//! give, no conditions and no memory operands (an address is on the stack, its offset an
//! immediate); which form takes which operands, `vun-isa-wasm32` says.

use vun_inline::ClassId;
use vun_items::Item;
use vun_items::lang::{
    arch_wasm32 as wasm, arch_wasm32_Class as class, arch_wasm32_Flow as flow, arch_wasm32_Form as form,
    arch_wasm32_Instruction as instruction, arch_wasm32_Size as size, arch_wasm32_Sized as sized,
};

use super::check::Held;
use super::need::Needs;
use super::parts::Widths;
use super::table::{Flow, Shape, Table};

pub(super) const SHAPE: Shape = Shape {
    instruction: wasm::Instruction,
    instruction_forms: instruction::forms,
    family: None,
    mnemonic: None,
    form_flow: form::flow,
    needs: Needs { form: form::requires, sized: None },
    flows: &[(flow::next, Flow::Next), (flow::ends, Flow::Ends)],
    size: wasm::Size,
    cond: None,
    class: wasm::Class,
    registers: wasm::Registers,
    choice: Some(wasm::Choice),
    widths: Widths { of: form::sizes, size: Some(sized::size), types: None, encoding: None },
    bits: &BITS,
    floats: true,
};

/// The bytes of the one vector type the architecture holds in a local.
const VECTOR_BYTES: u64 = 16;

/// The bits of each width.
const BITS: [(Item, u32); 2] = [(size::b32, 32), (size::b64, 64)];

/// The class of local a value of `bytes` bytes is held in: by its type, a float or an integer, at
/// 32 or 64 bits; narrower integers are held as 32. A size not known is a class of its own.
pub(super) fn class(table: &Table, bytes: Option<u64>, held: Held) -> Option<ClassId> {
    let Some(bytes) = bytes else { return table.class(class::unknown) };
    table.class(match (held, bytes > 4) {
        (Held::Vector, _) if bytes == VECTOR_BYTES => class::v128,
        (Held::Vector, _) => return None,
        (Held::Int, false) => class::i32,
        (Held::Int, true) => class::i64,
        (Held::Float, false) => class::f32,
        (Held::Float, true) => class::f64,
    })
}
