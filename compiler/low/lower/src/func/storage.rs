//! Where each local lives: nowhere, a stack slot, the data object of a top-level binding, or a
//! coroutine's frame.

use vun_codec::{Decode, Encode};
use vun_lir::{Address, Slot, Value};
use vun_mir::{Local, Role};
use vun_sema::kinds::Kinds;

use super::FnLower;
use crate::frame::FrameShape;
use crate::repr::Repr;
use crate::unlowered::Unlowered;

/// Where a local lives.
#[derive(Clone, Copy, Encode, Decode)]
pub(super) enum Storage {
    /// Nowhere: its type has size 0.
    None,
    Slot(Slot),
    /// The storage of the top-level binding it is: its data object, or the datum the linker
    /// provides under its symbol (`name: T = _;`).
    Global(Address),
    /// The calling thread's storage of the top-level binding it is, one per thread.
    Thread(vun_lir::DataId),
    /// The frame of the coroutine lowered, at this offset.
    Frame(u64),
}

impl FnLower<'_, '_, '_> {
    /// Where each local lives: nowhere when nothing reads or writes it or it has size 0, in the
    /// frame when `frame` holds it, in its data object or at its link symbol when it is a
    /// top-level binding, else in a stack slot.
    pub(super) fn storage(&mut self, params: &[Local], frame: Option<&FrameShape>) -> Result<(), Unlowered> {
        let body = self.body;
        let mut used = body.read();
        used.extend(params.iter().copied());
        used.extend(body.blocks.iter().flat_map(|(_, b)| b.stmts.iter().map(|s| s.place.local)));
        let guarded = self.guarded();
        for (local, decl) in body.locals.iter() {
            if !used.contains(&local) {
                self.slots.push(Storage::None);
                continue;
            }
            let ty = Self::typed(body, local, self.at)?;
            if let Some(offset) = frame.and_then(|f| f.stored[local]) {
                self.slots.push(Storage::Frame(offset));
                continue;
            }
            let storage = match (self.l.types.repr(ty, decl.origin)?, &decl.role) {
                (Repr::Zero, _) => Storage::None,
                // Immutable storage holding a cell is written through it all the same.
                (_, Role::Static(global) | Role::StaticMut(global))
                    if global.linked(self.l.types.cx).is_some() =>
                {
                    Storage::Global(self.l.linked(global, decl.origin)?)
                }
                (_, Role::Static(global)) if self.l.per_thread(global) => {
                    Storage::Thread(self.l.global(global, ty, true, decl.origin)?)
                }
                (_, Role::Static(global)) => {
                    let interior = self.l.types.cx.get::<Kinds>(&ty).interior;
                    Storage::Global(Address::Data(self.l.global(global, ty, interior, decl.origin)?))
                }
                (_, Role::StaticMut(global)) => {
                    Storage::Global(Address::Data(self.l.global(global, ty, true, decl.origin)?))
                }
                (Repr::Scalar(s), _) => {
                    let bytes = s.bytes(self.l.types.pointer_bits());
                    Storage::Slot(self.slot(local, bytes, bytes, &guarded)?)
                }
                (Repr::Memory(layout), _) => {
                    Storage::Slot(self.slot(local, layout.size, layout.align, &guarded)?)
                }
            };
            self.slots.push(storage);
        }
        Ok(())
    }

    /// The address of `local`'s storage; `None` for a local of size 0.
    pub(crate) fn local(&mut self, local: Local) -> Option<Value> {
        match self.slots[local] {
            Storage::None => None,
            Storage::Slot(slot) => Some(self.b.slot_addr(slot, self.at)),
            Storage::Global(address) => Some(self.b.addr(address, self.at)),
            Storage::Thread(data) => Some(self.b.thread_addr(data, self.at)),
            Storage::Frame(offset) => {
                let frame = self.frame?;
                Some(self.at_offset(frame, offset))
            }
        }
    }
}
