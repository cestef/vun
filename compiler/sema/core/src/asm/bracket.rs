//! Memory as the Arm architectures write it: a base register, then an offset in bytes, an index
//! register or a symbol's low bits (`mem(x1, 8)`, `mem(r0, r1)`).

use vun_hir::tree::ExprId;
use vun_inline::{Disp, Mem};

use super::check::Term;
use super::given::Addressing;

/// Memory from its parts: a base, then an offset in bytes, an index register or a symbol's low
/// bits.
pub(super) fn memory(parts: Vec<Term>) -> Option<Mem<ExprId, ExprId, ExprId>> {
    let mut parts = parts.into_iter();
    let Some(Term::Reg(base)) = parts.next() else { return None };
    let base = Some(base.base()?);
    let mem = match parts.next() {
        None => Mem { base, index: None, disp: Disp::Bytes(0) },
        Some(Term::Bytes(n)) => Mem { base, index: None, disp: Disp::Bytes(n) },
        Some(Term::Reg(index)) => Mem { base, index: Some((index.base()?, 1)), disp: Disp::Bytes(0) },
        Some(Term::Symbol(s)) => Mem { base, index: None, disp: Disp::Symbol(s, 0) },
        Some(Term::Scaled(..)) => return None,
    };
    parts.next().is_none().then_some(mem)
}
