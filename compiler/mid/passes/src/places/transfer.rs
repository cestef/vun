//! How statements change and check place states: reads need `init`, writes make it, borrows
//! need and leave the states their mode names (the table `borrows`).

use overt_diag::{Msg, errors};
use overt_grammar::BORROWS;
use overt_intern::Symbol;
use overt_mir::{Local, Operand, Place, Projection, Rvalue};
use overt_origin::OriginId;

use super::Analysis;
use super::states::{Key, State, States};

impl Analysis<'_> {
    pub(super) fn rvalue(&mut self, rvalue: &Rvalue, states: &mut States, origin: OriginId) {
        match rvalue {
            Rvalue::Use(o)
            | Rvalue::Unary { operand: o, .. }
            | Rvalue::Narrow { operand: o, .. }
            | Rvalue::Repeat { operand: o, .. } => self.operand(o, states, origin),
            Rvalue::Binary { left, right, .. } => {
                self.operand(left, states, origin);
                self.operand(right, states, origin);
            }
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
            Rvalue::IsVariant { place, .. } | Rvalue::Resume { frame: place, .. } => {
                self.read(place, states, origin);
            }
            Rvalue::Ref { mode, place } => self.borrow(*mode, place, states, origin),
            Rvalue::Comptime { .. } | Rvalue::Quote { .. } => {}
        }
    }

    /// A use of `operand`: a read, which moves a whole linear value out of its place.
    pub(super) fn operand(&mut self, operand: &Operand, states: &mut States, origin: OriginId) {
        if let Operand::Place(place) = operand {
            self.read(place, states, origin);
            if place.projection.is_empty() && self.linear.contains(&place.local) {
                states.locals[place.local] = State::Uninit;
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
    /// parameter's whole target becomes `init`.
    pub(super) fn write(&mut self, place: &Place, states: &mut States, origin: OriginId) {
        self.indices(place, states, origin);
        match (place.projection.as_slice(), self.path(place)) {
            ([], _) => states.write(place.local),
            ([Projection::Deref], _) if states.targets.contains_key(&place.local) => {
                states.targets.insert(place.local, State::Init);
            }
            (_, Some(path)) => {
                states.parts.insert((place.local, path), State::Init);
            }
            (_, None) => {}
        }
    }

    /// `&mode place`: the place must be in the state the mode needs, and is left in the state
    /// the mode gives.
    fn borrow(&mut self, mode: Option<Symbol>, place: &Place, states: &mut States, origin: OriginId) {
        let word = mode.map_or("", Symbol::as_str);
        let Some(&(.., before, after)) = BORROWS.iter().find(|(m, ..)| *m == word) else { return };
        if State::from(before) == State::Init {
            self.read(place, states, origin);
        }
        if place.projection.is_empty() {
            match State::from(after) {
                State::Init => states.write(place.local),
                state => states.locals[place.local] = state,
            }
        }
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
                Projection::Payload(at) => Some(Key::Payload(*at)),
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
                self.report(errors::PLACE_READ_UNINIT, origin, what().text(" is read before it is written"));
            }
            State::Ambiguous => self.report(
                errors::PLACE_AMBIGUOUS_JOIN,
                origin,
                what().text(" is written on some paths here and not on others"),
            ),
        }
    }
}
