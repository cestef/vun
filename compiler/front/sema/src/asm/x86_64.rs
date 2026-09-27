//! x86-64 (`lang.arch.x86_64`): registers named by width as `names` gives them and SSE registers
//! by their encoding, conditions as a suffix of the mnemonic (`jz`), memory as one sum
//! `[base + index * scale + disp]`, and each operand kind taking what the assembler's does.

use vun_hir::tree::ExprId;
use vun_inline::{ClassId, Disp, Mem, RegRef, SizeId};
use vun_intern::Symbol;
use vun_items::Item;
use vun_items::lang::{
    arch_x86_64 as x86, arch_x86_64_Class as class, arch_x86_64_Family as family, arch_x86_64_Flow as flow,
    arch_x86_64_Form as form, arch_x86_64_Instruction as instruction, arch_x86_64_Kind as kind,
    arch_x86_64_Member as member, arch_x86_64_Operand as operand, arch_x86_64_Reg as reg,
    arch_x86_64_Size as size,
};

use super::check::{Fit, Misfit, Term, Why};
use super::given::{Given, Word};
use super::table::{Fixed, Flow, Mnemonic, Shape, Table, variants};
use crate::env::{Env, Unevaluated};
use crate::target::Members;
use crate::ty::Value;

pub(super) const SHAPE: Shape = Shape {
    instruction: x86::Instruction,
    instruction_forms: instruction::forms,
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
        table.registers.insert(*name, reg);
        let Value::Struct { fields: widths, .. } = names else { return Err(Unevaluated::Unsupported) };
        for (width, text) in widths {
            let text = text.text().ok_or(Unevaluated::Unsupported)?;
            table.words.insert(Symbol::intern(&text), Word::Reg { reg, size: table.size(*width) });
        }
    }
    for (number, name) in variants(env, x86::Xmm)?.into_iter().enumerate() {
        let reg = RegRef { class: sse, number: u8::try_from(number).map_err(|_| Unevaluated::Unsupported)? };
        table.registers.insert(name, reg);
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

/// The width a variant of `Kind` carries: `rn(.b32)`.
fn carried(table: &Table, kind: &Value) -> Option<SizeId> {
    let Value::Variant { payload, .. } = kind else { return None };
    match payload.as_slice() {
        [Value::Variant { name, .. }] => table.size(*name),
        _ => None,
    }
}

/// A form taking a condition is spelled with each condition as a suffix: `j` as `jz`, `jnz`, ...
fn spellings(table: &Table, name: Symbol, form: &Value) -> Vec<(Mnemonic, Fixed)> {
    if !kinds(form).iter().any(|k| is(k, kind::cond)) {
        return vec![((name, None), Fixed::Nothing)];
    }
    let conds = table.conds.iter().enumerate().filter_map(|(i, cond)| Some((u8::try_from(i).ok()?, cond)));
    conds
        .map(|(i, cond)| {
            let spelled = Symbol::intern(&[name.as_str(), cond.as_str()].concat());
            ((spelled, None), Fixed::Cond(vun_inline::CondId(i)))
        })
        .collect()
}

/// The bits of the width `size`.
fn bits(table: &Table, size: SizeId) -> Option<u32> {
    let name = *table.sizes.get(usize::from(size.0))?;
    BITS.iter().find(|(item, _)| item.symbol() == name).map(|&(_, bits)| bits)
}

/// The width of a value of `bytes` bytes held in a general register.
pub(super) fn width(table: &Table, bytes: u64) -> Option<SizeId> {
    let bits = u32::try_from(bytes.checked_mul(8)?).ok()?;
    BITS.iter().find(|&&(_, b)| b == bits).and_then(|(item, _)| table.size(item.symbol()))
}

/// The class of register a value is held in: SSE for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::sse } else { class::general })
}

/// Whether the kind of an operand position has the instruction's width.
fn sized(kind: &Value) -> bool {
    [kind::r, kind::rm, kind::m].iter().any(|&k| is(kind, k))
}

/// The first width of `form` its operands agree with, and each operand as the form takes it.
pub(super) fn fit(table: &Table, form: &Value, fixed: Fixed, givens: &[Given]) -> Result<Fit, Misfit> {
    let kinds = kinds(form);
    // Each position's operand, with its place among those written: none for a fixed condition.
    let mut rest = givens.iter().enumerate();
    let mut operands = Vec::with_capacity(kinds.len());
    for kind in &kinds {
        match fixed {
            Fixed::Cond(cond) if is(kind, kind::cond) => operands.push((None, Given::Cond(cond))),
            _ => {
                let (at, given) = rest.next().ok_or(Misfit::Count)?;
                operands.push((Some(at), given.clone()));
            }
        }
    }
    if rest.next().is_some() {
        return Err(Misfit::Count);
    }
    let misfit = |at: Option<usize>, why| Misfit::Operand { at: at.unwrap_or_default(), why };
    let sizes = Members::of(form).ok().and_then(|m| m.get(form::sizes).ok().cloned());
    let sizes: Vec<SizeId> = sizes
        .and_then(Value::items)
        .unwrap_or_default()
        .iter()
        .filter_map(|s| match s {
            Value::Variant { name, .. } => table.size(*name),
            _ => None,
        })
        .collect();
    // The instruction's width is the one its registers of that width have.
    let widths = kinds.iter().zip(&operands).filter(|(k, _)| sized(k));
    let mut known = None;
    for (_, (at, given)) in widths {
        let Some(width) = given.size() else { continue };
        if known.is_some_and(|k| k != width) {
            return Err(misfit(*at, Why::Kind));
        }
        known = Some(width);
    }
    let size = match known {
        Some(size) => size,
        None if sizes.len() == 1 || !kinds.iter().any(sized) => *sizes.first().ok_or(Misfit::Count)?,
        None => return Err(Misfit::Unsized),
    };
    if !sizes.contains(&size) {
        return Err(Misfit::Width);
    }
    let op_bits = bits(table, size).ok_or(Misfit::Count)?;
    for (kind, (at, given)) in kinds.iter().zip(&operands) {
        takes(table, kind, size, op_bits, given).map_err(|why| misfit(*at, why))?;
    }
    Ok(Fit { size, operands: operands.into_iter().map(|(_, given)| given.operand()).collect() })
}

/// Whether the position of `kind` takes `given` in an instruction of `size`, of `op_bits`.
fn takes(table: &Table, kind: &Value, size: SizeId, op_bits: u32, given: &Given) -> Result<(), Why> {
    let general = class(table, false);
    let width = if sized(kind) { Some(size) } else { carried(table, kind) };
    let register = |want: Option<ClassId>| match given.class() {
        Some(class) if Some(class) != want => Err(Why::Class),
        Some(_) if given.size().is_some() && width.is_some() && given.size() != width => Err(Why::Kind),
        Some(_) => Ok(()),
        None => Err(Why::Kind),
    };
    let memory = matches!(given, Given::Mem(_));
    let imm = |fits: fn(i64, u32) -> bool| match given {
        Given::Imm(Some(v)) if held(*v, op_bits) && fits(*v, op_bits) => Ok(()),
        Given::Imm(_) => Err(Why::Wide),
        _ => Err(Why::Kind),
    };
    let name = |k: Item| is(kind, k);
    if name(kind::r) || name(kind::rn) {
        register(general)
    } else if name(kind::m) {
        if memory { Ok(()) } else { Err(Why::Kind) }
    } else if name(kind::rm) || name(kind::rmn) {
        if memory { Ok(()) } else { register(general) }
    } else if name(kind::x) {
        register(class(table, true))
    } else if name(kind::xmn) {
        if memory { Ok(()) } else { register(class(table, true)) }
    } else if name(kind::cl) {
        let rcx = table.registers.get(&reg::rcx.symbol()).copied();
        match given {
            Given::Reg { reg, size: Some(s) }
                if Some(*reg) == rcx && table.size(size::b8.symbol()) == Some(*s) =>
            {
                Ok(())
            }
            _ => register(general).and(Err(Why::Kind)),
        }
    } else if name(kind::i8) {
        imm(|v, bits| fits_signed(v, 8, bits))
    } else if name(kind::iz) {
        imm(|v, bits| fits_signed(v, bits.min(32), bits))
    } else if name(kind::u32) {
        imm(|v, bits| fits_unsigned(v, 32, bits))
    } else if name(kind::i64) {
        imm(|_, _| true)
    } else if name(kind::rel) {
        if matches!(given, Given::Label(_) | Given::Symbol(_)) { Ok(()) } else { Err(Why::Kind) }
    } else if name(kind::cond) {
        if matches!(given, Given::Cond(_)) { Ok(()) } else { Err(Why::Kind) }
    } else {
        Err(Why::Kind)
    }
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

/// Whether an operation of `bits` holds `value`, read as signed or as unsigned: a constant is
/// never silently cut to the bits the operation reads.
const fn held(value: i64, bits: u32) -> bool {
    bits >= 64 || (value >= -(1 << (bits - 1)) && value < (1 << bits))
}

/// The low `bits` bits of `value`.
const fn truncate(value: i64, bits: u32) -> u64 {
    if bits >= 64 { value.cast_unsigned() } else { value.cast_unsigned() & ((1 << bits) - 1) }
}

/// The low `bits` bits of `value`, sign-extended to 64.
const fn sign_extend(value: u64, bits: u32) -> i64 {
    if bits >= 64 {
        value.cast_signed()
    } else {
        let shift = 64 - bits;
        (value << shift).cast_signed() >> shift
    }
}

/// Whether an immediate of `imm_bits`, sign-extended to `op_bits`, gives `value` as an operation
/// of `op_bits` reads it, as the assembler encodes it.
const fn fits_signed(value: i64, imm_bits: u32, op_bits: u32) -> bool {
    truncate(value, op_bits) == truncate(sign_extend(truncate(value, imm_bits), imm_bits), op_bits)
}

/// Whether an immediate of `imm_bits`, zero-extended to `op_bits`, gives `value`.
const fn fits_unsigned(value: i64, imm_bits: u32, op_bits: u32) -> bool {
    truncate(value, op_bits) == truncate(value, if imm_bits < op_bits { imm_bits } else { op_bits })
}
