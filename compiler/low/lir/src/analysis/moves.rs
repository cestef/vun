//! The moves filling a jump's target parameters from its arguments: a parallel copy, sequenced so
//! that no parameter is written before every move reading it has read it. A cycle of moves is
//! broken by a temporary of the cycle's type; cycles of one type share it.

use crate::entity::Value;
use crate::func::Function;
use crate::inst::Target;
use crate::ty::Scalar;
use vun_codec::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Location {
    Value(Value),
    Temp(Scalar),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Move {
    pub to: Location,
    pub from: Location,
}

/// The moves of one jump, in the order they run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Moves(pub Vec<Move>);

impl Moves {
    /// The moves of the jump `target` of `func`; a parameter given itself does not move.
    #[must_use]
    pub fn of(func: &Function, target: &Target) -> Self {
        let params = &func.blocks[target.block].params;
        let mut pending: Vec<Move> = params
            .iter()
            .zip(&target.args)
            .filter(|(p, a)| p != a)
            .map(|(&p, &a)| Move { to: Location::Value(p), from: Location::Value(a) })
            .collect();
        let mut moves = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let free = pending.iter().position(|m| !pending.iter().any(|other| other.from == m.to));
            if let Some(i) = free {
                moves.push(pending.remove(i));
                continue;
            }
            // Every pending move is on a cycle: save one destination, and read it there.
            let saved = pending[0].to;
            let Location::Value(value) = saved else { unreachable!("only parameters are written") };
            let temp = Location::Temp(func.ty(value));
            moves.push(Move { to: temp, from: saved });
            for m in &mut pending {
                if m.from == saved {
                    m.from = temp;
                }
            }
        }
        Self(moves)
    }
}
