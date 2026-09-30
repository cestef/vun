//! Coroutine frames (spec 06, section 5): where a frame keeps which suspension point it is at,
//! its parameters, and the locals live across a `yield`, the frame's size being part of the
//! language. `Frames` gives the shape of each frame type.

mod live;

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;
use vun_lir::Scalar;
use vun_mir::{BlockId, Concrete, Local};
use vun_query::{Cx, Cycle, Query};
use vun_sema::layout::{Framer, Layout};
use vun_sema::target::TargetOf;
use vun_sema_ty::{Ty, TyKind};

use crate::repr::Types;
use crate::unlowered::{Unlowered, Why};

/// Where a frame keeps what it holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct FrameShape {
    /// The integer at offset 0 telling where the frame is: not started, suspended at a
    /// `yield`, or done.
    pub state: Scalar,
    pub layout: Layout,
    /// The offset of each local the frame holds.
    pub stored: IndexVec<Local, Option<u64>>,
    /// The blocks ending in a `yield`, in order.
    pub points: Vec<BlockId>,
}

/// Where a frame is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum FrameState {
    Start,
    /// Suspended at its `k`-th `yield`.
    Suspended(usize),
    /// Returned.
    Done,
}

impl FrameShape {
    /// How `state` is stored: 0 for the start, then each `yield` in order, then done.
    ///
    /// # Panics
    /// Never: a body has fewer `yield`s than `u128` has values.
    #[must_use]
    pub fn stored(&self, state: FrameState) -> u128 {
        let index = match state {
            FrameState::Start => 0,
            FrameState::Suspended(k) => k + 1,
            FrameState::Done => self.points.len() + 1,
        };
        u128::try_from(index).expect("states fit u128")
    }
}

/// The shape of the frames of a frame type.
pub struct Frames;

impl Query for Frames {
    const NAME: &'static str = "frames";
    type Key = Ty;
    type Value = Arc<Result<FrameShape, Unlowered>>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let TyKind::Frame(frame) = &**ty else { unreachable!("frames are shaped by their types") };
        let at = frame.literal.expr.origin;
        let shaped = || -> Result<FrameShape, Unlowered> {
            let instantiation = frame.instantiation();
            let body =
                cx.get::<Concrete>(&instantiation).map_err(|u| Unlowered::new(u.at(), Why::Unready(u)))?;
            let target = cx.get::<TargetOf>(&()).ok_or(Unlowered::new(at, Why::Untargeted))?;
            let types = Types::new(cx, &target).map_err(|bits| Unlowered::new(at, Why::Address(bits)))?;
            FrameShape::of(types, &instantiation, &body)
        };
        Arc::new(shaped())
    }

    fn cycle(_: &Cx<'_>, ty: &Ty, _: &Cycle) -> Self::Value {
        let TyKind::Frame(frame) = &**ty else { unreachable!("frames are shaped by their types") };
        Arc::new(Err(Unlowered::new(frame.literal.expr.origin, Why::Recursive)))
    }
}

/// Frame layouts for sema (`vun_sema::layout::Framers`), from [`Frames`].
pub struct Framing;

impl Framer for Framing {
    fn layout(&self, cx: &Cx<'_>, ty: &Ty) -> Option<Layout> {
        cx.get::<Frames>(ty).as_ref().as_ref().ok().map(|shape| shape.layout)
    }
}
