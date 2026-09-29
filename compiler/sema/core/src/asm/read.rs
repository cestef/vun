//! Reading a call's operands: values known at compile time as the architecture's words,
//! integers and addresses; labels as the instructions they mark; and the operand forms of
//! `lang.arch`: bindings with the register their value needs (`in(e)`), memory from its parts
//! (`mem(x1, 8)`) and its write-back (`pre`, `post`).

use std::collections::BTreeMap;

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::resolve::Res;
use vun_hir::tree::{Arg, BinderId, Entry, ExprId, ExprKind};
use vun_inline::{Bound, Disp, Line, Mem};
use vun_isa::Writeback;
use vun_primitives::arch::Arch as Form;
use vun_sema_ty::Value;

use super::check::{Checker, Term};
use super::given::Given;

/// The instruction each label marks.
pub(super) type Labels = BTreeMap<BinderId, Line>;

impl Checker<'_, '_, '_> {
    /// The operand form of `lang.arch` that `callee` calls, if it calls one.
    pub(super) fn form(&self, callee: ExprId) -> Option<Form> {
        self.forms.get(&callee).copied()
    }

    /// The arguments of the call `expr`, each written without a name; `None` for another
    /// expression.
    pub(super) fn args(&self, expr: ExprId) -> Option<(ExprId, Vec<ExprId>)> {
        let ExprKind::Call { callee, args } = &self.env.parsed.hir.exprs[expr].kind else { return None };
        let args = args.iter().map(|a| match a {
            Entry::One(Arg { name: None, value }) => Some(*value),
            _ => None,
        });
        Some((*callee, args.collect::<Option<_>>()?))
    }

    /// Reports an operand the architecture does not take, with `help`.
    fn unknown(&mut self, arg: ExprId, msg: Msg, help: Option<Msg>) -> Option<Given> {
        let origin = self.env.parsed.hir.exprs[arg].origin;
        let mut d = Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg).at(origin, None);
        if let Some(help) = help {
            d = d.child(ChildKind::Help, help);
        }
        self.diag.error(d);
        None
    }

    /// The operand `arg` gives, or `None` when it is none, which is reported.
    pub(super) fn given(&mut self, arg: ExprId, labels: &Labels) -> Option<Given> {
        if let Some((callee, args)) = self.args(arg)
            && let Some(form) = self.form(callee)
        {
            return self.formed(arg, callee, form, &args, labels);
        }
        if let Some(Res::Binder(b)) = self.env.parsed.resolution.names[arg]
            && let Some(&line) = labels.get(&b)
        {
            return Some(Given::Label(line));
        }
        match self.env.eval(arg) {
            Ok(Value::Int(n)) => Some(Given::Imm(n.to_i128().and_then(|n| i64::try_from(n).ok()))),
            Ok(Value::Variant { ty, name, payload }) if payload.is_empty() => {
                self.word(ty, name).or_else(|| {
                    let msg = Msg::new().text("this value names nothing of the architecture");
                    self.unknown(arg, msg, None)
                })
            }
            // What the linker places: a function, or an item borrowed.
            Ok(Value::Fn(_)) => Some(Given::Symbol(arg)),
            _ if matches!(self.env.parsed.hir.exprs[arg].kind, ExprKind::Borrow { .. }) => {
                Some(Given::Symbol(arg))
            }
            _ => {
                let msg = Msg::new().text("this is not an operand of an instruction");
                let help = Msg::new().text("a run-time value is passed in a register: ").code("in(e)");
                self.unknown(arg, msg, Some(help))
            }
        }
    }

    /// The operand the form `form` of `lang.arch`, called as `callee`, makes of `args`, written
    /// at `at`.
    fn formed(
        &mut self,
        at: ExprId,
        callee: ExprId,
        form: Form,
        args: &[ExprId],
        labels: &Labels,
    ) -> Option<Given> {
        match (form, args) {
            (Form::r#in | Form::out | Form::inout, [value]) => self.bound(form, *value),
            (Form::at, [register]) => {
                let ExprKind::Member { object, .. } = self.env.parsed.hir.exprs[callee].kind else {
                    return self.unknown(at, Msg::new().text("a register is named for a bound value"), None);
                };
                match self.given(object, labels)? {
                    Given::Bound { bound, .. } => self.pinned(bound, *register),
                    _ => self.unknown(object, Msg::new().text("a register is named for a bound value"), None),
                }
            }
            (Form::mem, parts) => {
                let mut terms = Vec::with_capacity(parts.len());
                for &part in parts {
                    terms.push(self.term(part, labels)?);
                }
                if let Some(mem) = self.arch.memory(terms) {
                    Some(Given::Mem { mem, writeback: Writeback::No })
                } else {
                    let msg = Msg::new().text("the architecture has no memory operand of these parts");
                    self.unknown(at, msg, None)
                }
            }
            (Form::pre, [memory]) => {
                if let Given::Mem { mem, writeback: Writeback::No } = self.given(*memory, labels)? {
                    Some(Given::Mem { mem, writeback: Writeback::Pre })
                } else {
                    self.unknown(at, Msg::new().text("only memory is written back"), None)
                }
            }
            (Form::post, [memory, offset]) => {
                if let (
                    Given::Mem {
                        mem: Mem { base, index: None, disp: Disp::Bytes(0) },
                        writeback: Writeback::No,
                    },
                    Given::Imm(Some(bytes)),
                ) = (self.given(*memory, labels)?, self.given(*offset, labels)?)
                {
                    let mem = Mem { base, index: None, disp: Disp::Bytes(bytes) };
                    Some(Given::Mem { mem, writeback: Writeback::Post })
                } else {
                    let msg = Msg::new().text("a base alone moves after the access, by bytes");
                    self.unknown(at, msg, None)
                }
            }
            _ => {
                let msg = Msg::new().code(form.name()).text(" is not an operand here");
                self.unknown(at, msg, None)
            }
        }
    }

    /// A part of memory: a register or a value read in one, an index scaled (`scaled(rcx, 8)`),
    /// bytes, or an address the linker places.
    fn term(&mut self, part: ExprId, labels: &Labels) -> Option<Term> {
        if let Some((callee, args)) = self.args(part)
            && self.form(callee) == Some(Form::scaled)
            && let [index, scale] = args.as_slice()
        {
            let index = self.given(*index, labels)?;
            let Ok(Value::Int(scale)) = self.env.eval(*scale) else {
                return self
                    .unknown(part, Msg::new().text("a scale is an integer"), None)
                    .map(|_| Term::Bytes(0));
            };
            return Some(Term::Scaled(index, i64::try_from(scale.to_i128()?).ok()?));
        }
        match self.given(part, labels)? {
            Given::Imm(Some(bytes)) => Some(Term::Bytes(bytes)),
            Given::Symbol(s) => Some(Term::Symbol(s)),
            given @ (Given::Reg { .. } | Given::Bound { bound: Bound::In { .. }, .. }) => {
                Some(Term::Reg(given))
            }
            _ => {
                self.unknown(part, Msg::new().text("this is no part of memory"), None).map(|_| Term::Bytes(0))
            }
        }
    }

    /// `in(value)`, `out(value)` or `inout(value)`: in a register of the class and width the
    /// value's type needs.
    fn bound(&mut self, form: Form, value: ExprId) -> Option<Given> {
        let (class, size) = *self.bound.get(&value)?;
        let bound = match form {
            Form::r#in => Bound::In { value, reg: None },
            Form::out => Bound::Out { place: value, reg: None },
            _ => Bound::InOut { place: value, reg: None },
        };
        Some(Given::Bound { bound, class, size })
    }

    /// `bound` in the register `register` names (`in(e).at(rdi)`), of its class, and of its width
    /// where its name gives one.
    fn pinned(&mut self, bound: Bound<ExprId, ExprId>, register: ExprId) -> Option<Given> {
        let value = match &bound {
            Bound::In { value, .. } => *value,
            Bound::Out { place, .. } | Bound::InOut { place, .. } => *place,
        };
        let named = match self.env.eval(register) {
            Ok(Value::Variant { ty, name, .. }) => self.word(ty, name),
            _ => None,
        };
        let Some(Given::Reg { reg, size }) = named else {
            return self.unknown(register, Msg::new().text("this is no register"), None);
        };
        let size = size.or_else(|| self.bound.get(&value)?.1);
        Some(Given::Bound { bound: bound.at(reg), class: reg.class, size })
    }
}
