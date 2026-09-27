//! 64-bit ARM (`lang.arch.arm64`): general registers named by width as `names` gives them (`w0`,
//! `x0`), floating-point ones as `fp_names` does (`s0`, `d0`), conditions, barriers and system registers as words,
//! a condition after the mnemonic's `.` (`b.eq`) and an access width as its suffix (`ldrb`), and
//! memory as `[base]`, `[base, offset]` or `[base, index]`; which form takes which operands,
//! `vun-isa-arm64` says.

use vun_hir::tree::ExprId;
use vun_inline::{ClassId, CondId, Disp, Mem, RegRef};
use vun_intern::Symbol;
use vun_isa::{ChoiceId, Fixed};
use vun_items::Item;
use vun_items::lang::{
    arch_arm64 as a, arch_arm64_Arg as arg, arch_arm64_Choice as choice, arch_arm64_Class as class,
    arch_arm64_Condition as condition, arch_arm64_Family as family, arch_arm64_Flow as flow,
    arch_arm64_Form as form, arch_arm64_Instruction as instruction, arch_arm64_Kind as kind,
    arch_arm64_Member as member, arch_arm64_Names as names, arch_arm64_Size as size,
    arch_arm64_Sized as sized,
};

use super::check::Term;
use super::given::{Addressing, Word};
use super::table::{Flow, Mnemonic, Shape, Table, variants};
use crate::env::{Env, Unevaluated};
use crate::target::Members;
use crate::ty::Value;

pub(super) const SHAPE: Shape = Shape {
    instruction: a::Instruction,
    instruction_forms: instruction::forms,
    instruction_mnemonic: Some(instruction::mnemonic),
    family: a::Family,
    family_forms: family::forms,
    members: family::members,
    member_name: member::name,
    form_name: form::name,
    form_flow: form::flow,
    flows: [(flow::next, Flow::Next), (flow::jump, Flow::Jump), (flow::ends, Flow::Ends)],
    size: a::Size,
    cond: a::Cond,
    class: a::Class,
    registers,
    spellings,
    bits: &BITS,
};

/// The bits of each width.
const BITS: [(Item, u32); 4] = [(size::b8, 8), (size::b16, 16), (size::b32, 32), (size::b64, 64)];

/// The width each field of `Names` names a general register at.
const NAME_WIDTHS: [(Item, Item); 2] = [(names::w, size::b32), (names::x, size::b64)];

/// The enum of the choices of each kind of `Choice`.
const CHOICES: [(Item, Item); 2] = [(choice::barrier, a::Barrier), (choice::sys, a::SysReg)];

/// A number of the table, as an index of its enum.
fn number(i: usize) -> Result<u8, Unevaluated> {
    u8::try_from(i).map_err(|_| Unevaluated::Unsupported)
}

/// General registers by each width's name, floating-point ones by each of theirs, and conditions
/// and barriers by their own.
fn registers(env: &Env<'_, '_>, table: &mut Table) -> Result<(), Unevaluated> {
    let (Some(general), Some(fp)) = (table.class(class::general), table.class(class::fp)) else {
        return Err(Unevaluated::Unsupported);
    };
    for (encodings, names, class, widths) in
        [(a::Reg, a::names, general, true), (a::V, a::fp_names, fp, false)]
    {
        let encodings = variants(env, encodings)?;
        let Value::Struct { fields, .. } = env.item(names)? else { return Err(Unevaluated::Unsupported) };
        for (name, spelled) in &fields {
            let at = encodings.iter().position(|e| e == name).ok_or(Unevaluated::Unsupported)?;
            let reg = RegRef { class, number: number(at)? };
            let Value::Struct { fields: spelled, .. } = spelled else { return Err(Unevaluated::Unsupported) };
            for (width, text) in spelled {
                let text = text.text().ok_or(Unevaluated::Unsupported)?;
                // A general register's names are `w` and `x`; a floating-point one's, its widths.
                let size = if widths {
                    NAME_WIDTHS
                        .iter()
                        .find(|(f, _)| f.symbol() == *width)
                        .and_then(|(_, s)| table.size(s.symbol()))
                } else {
                    table.size(*width)
                };
                table.words.insert(Symbol::intern(&text), Word::Reg { reg, size });
            }
        }
    }
    for (i, cond) in table.conds.clone().into_iter().enumerate() {
        table.words.insert(cond, Word::Cond(CondId(number(i)?)));
    }
    // The choices a position names, each kind of the table's `Choice` an enum of its own.
    let kinds = variants(env, a::Choice)?;
    for (kind, choices) in CHOICES {
        let of = ChoiceId(number(
            kinds.iter().position(|&k| k == kind.symbol()).ok_or(Unevaluated::Unsupported)?,
        )?);
        for (i, choice) in variants(env, choices)?.into_iter().enumerate() {
            table.words.insert(choice, Word::Choice { of, index: number(i)? });
        }
    }
    Ok(())
}

/// A member of a struct value, by its handle.
fn field(value: &Value, item: Item) -> Option<Value> {
    Members::of(value).ok()?.get(item).ok().cloned()
}

/// Whether `value` is the variant `item`, and its payload.
fn variant(value: &Value, item: Item) -> Option<Vec<Value>> {
    match value {
        Value::Variant { name, payload, .. } if *name == item.symbol() => Some(payload.clone()),
        _ => None,
    }
}

/// A form is spelled as its mnemonic, or with the suffix of an access width (`ldrb`), and one
/// whose condition is a suffix with each condition after a `.` (`b.eq`).
fn spellings(table: &Table, name: Symbol, form: &Value) -> Vec<(Mnemonic, Fixed)> {
    let sizes = field(form, form::sizes).and_then(Value::items).unwrap_or_default();
    let mut widths = Vec::new();
    for s in &sizes {
        let suffix = field(s, sized::suffix).and_then(|t| t.text()).unwrap_or_default();
        let width = field(s, sized::size).and_then(|v| match v {
            Value::Variant { name, .. } => table.size(name),
            _ => None,
        });
        let spelled = if suffix.is_empty() {
            (name, Fixed::default())
        } else {
            (Symbol::intern(&[name.as_str(), &suffix].concat()), Fixed { cond: None, size: width })
        };
        if !widths.contains(&spelled) {
            widths.push(spelled);
        }
    }
    if widths.is_empty() {
        widths.push((name, Fixed::default()));
    }
    let args = field(form, form::args).and_then(Value::items).unwrap_or_default();
    let suffixed = args.iter().filter_map(|a| field(a, arg::kind)).any(|k| {
        variant(&k, kind::cond)
            .is_some_and(|p| p.first().is_some_and(|c| variant(c, condition::suffix).is_some()))
    });
    let conds: Vec<Option<(u8, Symbol)>> = if suffixed {
        table.conds.iter().enumerate().filter_map(|(i, &c)| Some(Some((u8::try_from(i).ok()?, c)))).collect()
    } else {
        vec![None]
    };
    let mut out = Vec::new();
    for &(spelled, fixed) in &widths {
        for cond in &conds {
            out.push(match cond {
                Some((i, c)) => ((spelled, Some(*c)), Fixed { cond: Some(CondId(*i)), ..fixed }),
                None => ((spelled, None), fixed),
            });
        }
    }
    out
}

/// The class of register a value is held in: floating-point for a float, general otherwise.
pub(super) fn class(table: &Table, float: bool) -> Option<ClassId> {
    table.class(if float { class::fp } else { class::general })
}

/// Memory from its parts: a base, then an offset in bytes, an index register or a symbol's low
/// bits.
pub(super) fn memory(parts: Vec<Vec<Term>>) -> Option<Mem<ExprId, ExprId, ExprId>> {
    let mut parts = parts.into_iter().map(|p| <[Term; 1]>::try_from(p).ok().map(|[t]| t));
    let Some(Term::Reg(base)) = parts.next()? else { return None };
    let base = Some(base.base()?);
    let mem = match parts.next() {
        None => Mem { base, index: None, disp: Disp::Bytes(0) },
        Some(Some(Term::Bytes(n))) => Mem { base, index: None, disp: Disp::Bytes(n) },
        Some(Some(Term::Reg(index))) => Mem { base, index: Some((index.base()?, 1)), disp: Disp::Bytes(0) },
        Some(Some(Term::Symbol(s))) => Mem { base, index: None, disp: Disp::Symbol(s, 0) },
        Some(_) => return None,
    };
    parts.next().is_none().then_some(mem)
}
