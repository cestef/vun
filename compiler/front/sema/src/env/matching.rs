//! `match` at compile time (spec 01, section 7): the first arm whose pattern takes the value of
//! the scrutinee and whose guard holds gives the match its value, the binders of the pattern bound
//! to the parts they take.

use vun_grammar::TokenKind;
use vun_hir::tree::{Arm, BinderId, Entry, ExprId, List, Name, PatId, PatKind};

use super::operator::Operand;
use super::{Env, Unevaluated};
use crate::int::Interval;
use crate::ty::{Ty, Value};

impl Env<'_, '_> {
    /// `match scrutinee { arms }`.
    pub(super) fn matching(&self, scrutinee: ExprId, arms: &List<Arm>) -> Result<Value, Unevaluated> {
        let value = self.eval(scrutinee)?;
        let ty = self.type_of(scrutinee);
        for arm in arms {
            let Entry::One(arm) = arm else { return Err(Unevaluated::Unsupported) };
            let mut bound = Vec::new();
            if !self.takes(arm.pat, &value, ty, &mut bound)? {
                continue;
            }
            let mut env = self.clone();
            env.bindings.extend(bound);
            if let Some(guard) = arm.guard
                && !env.holds(&env.eval(guard)?)?
            {
                continue;
            }
            return arm.value.map_or_else(|| Ok(Value::Tuple(Vec::new())), |v| env.eval(v));
        }
        // Typing checks that the arms cover every value: one that none takes is not typed yet.
        Err(Unevaluated::Unsupported)
    }

    /// Whether the pattern `pat` takes `value`, of the type typing found, `ty`, if it did; the
    /// binders it binds pushed on `bound`.
    fn takes(
        &self,
        pat: PatId,
        value: &Value,
        ty: Option<Ty>,
        bound: &mut Vec<(BinderId, Value)>,
    ) -> Result<bool, Unevaluated> {
        let hir = &self.parsed.hir;
        match (&hir.pats[pat].kind, value) {
            (PatKind::Bind(binder), _) => {
                bound.push((*binder, value.clone()));
                Ok(true)
            }
            (PatKind::Hole, _) => Ok(true),
            (PatKind::At { binder, pat }, _) => {
                bound.push((*binder, value.clone()));
                self.takes(*pat, value, ty, bound)
            }
            (PatKind::Or(alternatives), _) => {
                for &alternative in alternatives {
                    let mut taken = Vec::new();
                    if self.takes(alternative, value, ty, &mut taken)? {
                        bound.extend(taken);
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            (PatKind::Tuple(items), Value::Tuple(parts)) => self.all(items, parts, bound),
            (PatKind::Variant { name, args }, Value::Variant { name: variant, payload, .. }) => {
                if self.name(name) != Some(*variant) {
                    return Ok(false);
                }
                args.as_ref().map_or(Ok(true), |args| self.all(args, payload, bound))
            }
            (PatKind::Struct { fields, .. }, Value::Struct { fields: values, .. }) => {
                for (at, field) in fields.iter().enumerate() {
                    let Entry::One((name, pat)) = field else { return Err(Unevaluated::Unsupported) };
                    let held = match name {
                        Some(Name::Plain(name)) => values.iter().find(|(n, _)| n == name).map(|(_, v)| v),
                        Some(_) => return Err(Unevaluated::Unsupported),
                        None => values.get(at).map(|(_, v)| v),
                    };
                    match (&hir.pats[*pat].kind, held) {
                        (PatKind::Rest(None), _) => {}
                        (_, Some(held)) if self.takes(*pat, held, None, bound)? => {}
                        (_, Some(_)) => return Ok(false),
                        (_, None) => return Err(Unevaluated::Unsupported),
                    }
                }
                Ok(true)
            }
            (PatKind::Range { lo, hi, inclusive }, Value::Int(_)) => {
                let op = if *inclusive { TokenKind::DotDotEq } else { TokenKind::DotDot };
                let hi = hi.map(|h| self.eval(h)).transpose()?;
                let bounds = Value::range(op, &self.eval(*lo)?, hi.as_ref())?.ranged();
                let Value::Range(items) = bounds else { return Err(Unevaluated::Unsupported) };
                let Value::Int(n) = value else { return Err(Unevaluated::Unsupported) };
                Ok(items.is_some_and(|items| items.contains(&Interval::singleton(n.clone()))))
            }
            // A value pattern `v` is `v == x`, as at run time (spec 02, section 4).
            (PatKind::Value(expr), _) => {
                let equal = self.binary(
                    TokenKind::EqEq,
                    Operand { value: value.clone(), ty },
                    Operand { value: self.eval_as(*expr, ty)?, ty },
                )?;
                self.holds(&equal)
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// Whether the patterns `pats` take the parts `values` in order, a rest `..` taking the parts
    /// the others leave between them.
    fn all(
        &self,
        pats: &List<PatId>,
        values: &[Value],
        bound: &mut Vec<(BinderId, Value)>,
    ) -> Result<bool, Unevaluated> {
        let pats = pats
            .iter()
            .map(|p| match p {
                Entry::One(p) => Ok(*p),
                Entry::Staged(_) => Err(Unevaluated::Unsupported),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let hir = &self.parsed.hir;
        let rest = pats.iter().position(|&p| matches!(hir.pats[p].kind, PatKind::Rest(_)));
        let (before, after) = match rest {
            Some(at) => (&pats[..at], &pats[at + 1..]),
            None if pats.len() == values.len() => (&pats[..], &[][..]),
            None => return Ok(false),
        };
        if before.len() + after.len() > values.len() {
            return Ok(false);
        }
        let tail = &values[values.len() - after.len()..];
        for (&pat, held) in before.iter().zip(values).chain(after.iter().zip(tail)) {
            if !self.takes(pat, held, None, bound)? {
                return Ok(false);
            }
        }
        if let Some(PatKind::Rest(Some(binder))) = rest.map(|at| &hir.pats[pats[at]].kind) {
            let taken = values[before.len()..values.len() - after.len()].to_vec();
            bound.push((*binder, Value::Tuple(taken)));
        }
        Ok(true)
    }
}
