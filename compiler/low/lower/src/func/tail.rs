//! Guaranteed tail calls (spec 09, section 3): a call that is the function's last instruction,
//! returning what it gives. The frame is the callee's, so what it passes may not be in it.

use vun_lir::{CallKind, Callee, InstKind, Signature, Value};

use super::{FnLower, Tailed};
use crate::unlowered::{Form, Unlowered};

impl FnLower<'_, '_, '_> {
    /// Calls `callee` as the last instruction of the function, which returns what it gives.
    pub(super) fn tail(
        &mut self,
        callee: Callee,
        mut args: Vec<Value>,
        signature: &Signature,
    ) -> Result<(), Unlowered> {
        let own = self.b.signature();
        let shared = own.conv == signature.conv
            && own.convention == signature.convention
            && own.result == signature.result;
        if !shared {
            return Err(Unlowered::unsupported(self.at, Form::Tail));
        }
        self.unguard()?;
        self.reuse_received(signature, &mut args)?;
        let value =
            self.b.push(InstKind::Call { callee, args, kind: CallKind::Tail }, signature.returns(), self.at);
        self.tail = Some(Tailed(value));
        Ok(())
    }

    /// Passes each value in memory of Vun's convention in the memory the function received for
    /// one of its own, the first not taken that holds it: that memory is its caller's and read no
    /// more, where a slot of this frame is gone once the callee runs. The values are copied there
    /// once all are written, as one may be read from another.
    fn reuse_received(&mut self, callee: &Signature, args: &mut [Value]) -> Result<(), Unlowered> {
        let mut received = self.b.signature().memory().collect::<Vec<_>>().into_iter();
        let mut moves = Vec::new();
        for (at, size, align) in callee.memory() {
            let fits = |&(_, held, aligned): &(usize, u64, u64)| held >= size && aligned >= align;
            let (own, ..) = received.find(fits).ok_or(Unlowered::unsupported(self.at, Form::Tail))?;
            moves.push((at, self.received[own], size));
        }
        for (at, to, size) in moves {
            self.b.copy(to, args[at], size, self.at);
            args[at] = to;
        }
        Ok(())
    }
}
