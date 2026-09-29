//! Memory as the Arm architectures write it: parts between brackets, a base register then an
//! offset in bytes, an index register or a symbol's low bits (`[x1, 8]`, `[r0, r1]`).

use vun_hir::tree::ExprId;
use vun_inline::{Disp, Mem};

use super::check::Term;
use super::given::Addressing;

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
