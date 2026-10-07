//! Guaranteed tail calls (spec 09, section 3): a call that is the function's last instruction,
//! returning what it gives. The frame is the callee's, so what it passes may not be in it.

use vun_lir::{CallKind, Callee, Held, InstKind, Reuse, Signature, Value};

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

    /// Passes each value held in memory in the memory the function received for one of its own,
    /// the first not taken that holds it: that memory is its caller's and read no more, where a
    /// slot of this frame is gone once the callee runs. Vun's convention needs it; a record of
    /// the C convention is passed on so where the target's convention would have the callee
    /// read a copy in the frame being left, and stays where none is received that fits. The
    /// values are copied there once all are written, as one may be read from another.
    fn reuse_received(&mut self, callee: &Signature, args: &mut [Value]) -> Result<(), Unlowered> {
        let mut received = self.b.signature().held().collect::<Vec<_>>().into_iter();
        let mut moves = Vec::new();
        for held in callee.held() {
            let fits = |own: &Held| own.size >= held.size && own.align >= held.align;
            match (received.find(fits), held.reuse) {
                (Some(own), _) => moves.push((held.at, self.received[own.at], held.size)),
                (None, Reuse::Must) => return Err(Unlowered::unsupported(self.at, Form::Tail)),
                (None, Reuse::May) => {}
            }
        }
        for (at, to, size) in moves {
            self.b.copy(to, args[at], size, self.at);
            args[at] = to;
        }
        Ok(())
    }
}
