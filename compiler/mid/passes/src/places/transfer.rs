//! How statements change and check place states: reads need `init`, writes make it, borrows
//! need and leave the states their mode names (the table `borrows`).

use vun_desugar::Borrow;
use vun_diag::{Msg, errors};
use vun_intern::Symbol;
use vun_mir::{BlockId, Local, Operand, Place, Projection, Rvalue};
use vun_origin::OriginId;

use super::Analysis;
use super::leak::Stop;
use super::states::{Key, State, States};

impl Analysis<'_> {
    pub(super) fn rvalue(&mut self, rvalue: &Rvalue, states: &mut States, origin: OriginId) {
        match rvalue {
            Rvalue::Use(o)
            | Rvalue::Not { operand: o }
            | Rvalue::Narrow { operand: o, .. }
            | Rvalue::Repeat { operand: o, .. } => self.operand(o, states, origin),
            Rvalue::Range { lo, hi, .. } => {
                self.operand(lo, states, origin);
                if let Some(hi) = hi {
                    self.operand(hi, states, origin);
                }
            }
            Rvalue::Aggregate { parts, .. } => {
                for part in parts {
                    self.operand(part, states, origin);
                }
            }
            Rvalue::Call { callee, args } => {
                self.operand(callee, states, origin);
                for arg in args {
                    self.operand(arg, states, origin);
                }
            }
            // Its inputs and in-out places are read before any of its outputs is written.
            Rvalue::Asm(asm) => {
                for operand in rvalue.operands() {
                    self.operand(operand, states, origin);
                }
                for (place, _) in asm.outputs().filter(|&(_, reads)| reads) {
                    self.read(place, states, origin);
                }
                for (place, _) in asm.outputs() {
                    self.write(place, states, origin);
                }
            }
            Rvalue::IsVariant { place, .. } | Rvalue::Resume { frame: place, .. } => {
                self.read(place, states, origin);
            }
            Rvalue::Ref { mode, place } => self.borrow(*mode, place, states, origin),
            // Ending a frame consumes it.
            Rvalue::End { frame } => self.operand(&Operand::Place(frame.clone()), states, origin),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } | Rvalue::Splice { .. } | Rvalue::Dead => {}
        }
    }

    /// A use of `operand`: a read, which moves a linear value, whole or a linear part, out of
    /// its place.
    pub(super) fn operand(&mut self, operand: &Operand, states: &mut States, origin: OriginId) {
        if let Operand::Place(place) = operand {
            self.read(place, states, origin);
            if let Some(path) = self.path(place)
                && self.linear.holds(place.local, &path)
            {
                states.set(place.local, &path, State::Uninit);
            }
        }
    }

    /// A read of `place`: the part it names, or a borrow parameter's target, must be `init`, as
    /// must the locals its indices read.
    fn read(&mut self, place: &Place, states: &States, origin: OriginId) {
        let target = match place.projection.first() {
            Some(Projection::Deref) => states.targets.get(&place.local).copied(),
            _ => None,
        };
        let path = self.path(place);
        let state = target.unwrap_or_else(|| states.of(place.local, path.as_deref()));
        self.require(place.local, state, origin);
        self.indices(place, states, origin);
    }

    /// A write to `place`: a whole local, a part named without running the body, or a borrow
    /// parameter's whole target becomes `init`. A linear value it may still hold leaks.
    pub(super) fn write(&mut self, place: &Place, states: &mut States, origin: OriginId) {
        self.indices(place, states, origin);
        match (place.projection.as_slice(), self.path(place)) {
            ([Projection::Deref], _) if states.targets.contains_key(&place.local) => {
                states.targets.insert(place.local, State::Init);
            }
            (_, Some(path)) => {
                if self.holds(states, place.local, &path) {
                    self.leak(place.local, origin, Stop::Overwritten);
                }
                states.set(place.local, &path, State::Init);
            }
            (_, None) => {}
        }
    }

    /// Whether a linear part of `local` within `path`, or the whole when it is empty, may still
    /// hold a value.
    pub(super) fn holds(&self, states: &States, local: Local, path: &[Key]) -> bool {
        let leaves = self.linear.leaves(local).iter().filter(|leaf| leaf.starts_with(path));
        leaves.map(|leaf| states.of(local, Some(leaf))).any(|state| state != State::Uninit)
    }

    /// `&mode place`: the place must be in the state the mode needs, and is left in the state
    /// the mode gives.
    fn borrow(&mut self, mode: Borrow, place: &Place, states: &mut States, origin: OriginId) {
        let (before, after) = (mode.before(), mode.after());
        if State::from(before) == State::Init {
            self.read(place, states, origin);
        }
        // A mode that leaves the state it needs changes nothing: a borrow of an `uninit` place
        // is reported, not initialized.
        if place.projection.is_empty() && before != after {
            states.set(place.local, &[], State::from(after));
        }
    }

    /// A deferred statement at an exit runs when every linear local it consumes is `init`, and
    /// is skipped when one was moved; one moved on some paths here and not on others leaves it
    /// undecided (spec 05, section 3), which is reported once.
    pub(super) fn deferred(
        &mut self,
        consumes: &[Local],
        run: BlockId,
        skip: BlockId,
        states: &mut States,
        origin: OriginId,
    ) -> BlockId {
        if consumes.iter().all(|&l| states.locals[l] == State::Init) {
            return run;
        }
        // Reported once here, not again as a leak at the return.
        let ambiguous: Vec<Local> =
            consumes.iter().copied().filter(|&l| states.locals[l] == State::Ambiguous).collect();
        for local in ambiguous {
            states.locals[local] = State::Uninit;
            let name = self.body.locals[local].name.map_or("", Symbol::as_str);
            let msg = Msg::new()
                .code(name)
                .text(" is moved on some paths to this exit and not on others, so whether its ")
                .code("defer")
                .text(" runs here is unknown");
            let help = Msg::new()
                .text("consume it on every path to this exit or on none, so that the ")
                .code("defer")
                .text(" runs on all of them or is skipped on all");
            self.advise(errors::PLACE_AMBIGUOUS_JOIN, origin, msg, help);
        }
        skip
    }

    /// The locals the indices of `place` read must be `init`.
    fn indices(&mut self, place: &Place, states: &States, origin: OriginId) {
        for p in &place.projection {
            if let Projection::Index(index) = p {
                self.require(*index, states.locals[*index], origin);
            }
        }
    }

    /// The path of the part `place` names within its local, when no `.*` or dynamic index is on
    /// the way.
    fn path(&self, place: &Place) -> Option<Vec<Key>> {
        place
            .projection
            .iter()
            .map(|p| match p {
                Projection::Field(name) => Some(Key::Field(*name)),
                Projection::Position(at) => Some(Key::Position(*at)),
                Projection::Payload { variant, at } => Some(Key::Payload { variant: *variant, at: *at }),
                Projection::Index(index) => self.constants.get(index).map(|&n| Key::Item(n)),
                Projection::Deref => None,
            })
            .collect()
    }

    fn require(&mut self, local: Local, state: State, origin: OriginId) {
        let name = self.body.locals[local].name;
        let what = || name.map_or_else(|| Msg::new().text("this value"), |n| Msg::new().code(n.as_str()));
        match state {
            State::Init => {}
            State::Uninit => {
                let help = Msg::new().text("give it a value where it is declared, or write it before this");
                self.advise(
                    errors::PLACE_READ_UNINIT,
                    origin,
                    what().text(" is read before it is written"),
                    help,
                );
            }
            State::Ambiguous => {
                let msg = what().text(" is written on some paths to here and not on others");
                let help = Msg::new()
                    .text("write it on every path to here, or give it a value where it is declared");
                self.advise(errors::PLACE_AMBIGUOUS_JOIN, origin, msg, help);
            }
        }
    }
}
