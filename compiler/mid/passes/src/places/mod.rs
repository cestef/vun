//! Place-state analysis (spec 05, section 2): every place is `uninit` or `init`, and each read,
//! borrow and return is checked against the state its place has on every path to it. Parts
//! reached by fields and constant indices are tracked on their own; a dynamic index needs the
//! whole written. A borrow parameter's target starts in the state its mode needs before, and must
//! reach the state its mode leaves after (the table `borrows`). A read of a whole linear value
//! moves it, leaving its place `uninit`, and a function returns only with every linear place
//! `uninit` (spec 05, section 1: kinds).

mod moved;
mod states;
mod transfer;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use vun_desugar::BORROWS;
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg, errors};
use vun_index::{Idx, IndexVec};
use vun_intern::Symbol;
use vun_mir::{BlockId, Body, Local, Mir, Operand, Role, Rvalue, TerminatorKind};
use vun_origin::OriginId;
use vun_query::{Cx, Query};
use vun_sema::eval::Instantiation;
use vun_sema::kinds::Kinds;
use vun_sema::ty::Value;

use moved::Moved;
use states::{State, States};

/// The place-state reports of an instantiation's body; `None` when it has no MIR yet.
pub struct PlaceStates;

impl Query for PlaceStates {
    const NAME: &'static str = "place_states";
    type Key = Instantiation;
    type Value = Option<DiagCtxt>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let body = cx.get::<Mir>(i).ok()?;
        let linear =
            body.locals.iter().filter(|(_, d)| d.ty.is_some_and(|t| cx.get::<Kinds>(&t).is_linear()));
        let linear = linear.filter(|(_, d)| d.role == Role::Held).map(|(local, _)| local);
        let linear: BTreeSet<Local> = linear.filter(|&l| l != body.ret).collect();
        let moved = Moved::find(cx, &body, |l| linear.contains(&l));
        Some(Analysis::new(&body, linear, moved).run())
    }
}

struct Analysis<'b> {
    body: &'b Body,
    /// Each borrow parameter, with the states its mode needs before and leaves after.
    targets: BTreeMap<Local, (State, State)>,
    /// The locals assigned once, a constant integer: the indices a part can be named by.
    constants: BTreeMap<Local, i128>,
    /// The locals holding linear values, which a read moves and a return must find `uninit`.
    linear: BTreeSet<Local>,
    /// The linear values moved out of borrows, by the local they land in.
    moved: Vec<(Local, OriginId)>,
    diag: DiagCtxt,
    reported: BTreeSet<(DiagId, OriginId)>,
    /// Whether transfers report what they find: only once the states are fixed.
    reporting: bool,
}

impl<'b> Analysis<'b> {
    fn new(body: &'b Body, linear: BTreeSet<Local>, moved: Vec<(Local, OriginId)>) -> Self {
        let targets = body
            .params
            .iter()
            .filter_map(|&p| {
                let (mode, ..) = body.locals[p].ty?.borrowing()?;
                let &(_, _, before, after, ..) = BORROWS.iter().find(|(m, ..)| *m == mode)?;
                Some((p, (State::from(before), State::from(after))))
            })
            .collect();
        Self {
            body,
            targets,
            constants: Self::constants(body),
            linear,
            moved,
            diag: DiagCtxt::new(),
            reported: BTreeSet::new(),
            reporting: false,
        }
    }

    /// The locals of `body` assigned exactly once, and to a constant integer.
    fn constants(body: &Body) -> BTreeMap<Local, i128> {
        let mut assigned: BTreeMap<Local, Option<i128>> = BTreeMap::new();
        for (_, block) in body.blocks.iter() {
            for stmt in block.stmts.iter().filter(|s| s.value != Rvalue::Dead) {
                let value = match (&stmt.value, stmt.place.projection.is_empty()) {
                    (Rvalue::Use(Operand::Const(Value::Int(n))), true) => n.to_i128(),
                    _ => None,
                };
                assigned.entry(stmt.place.local).and_modify(|v| *v = None).or_insert(value);
            }
        }
        assigned.into_iter().filter_map(|(local, value)| Some((local, value?))).collect()
    }

    fn run(mut self) -> DiagCtxt {
        let mut at: IndexVec<BlockId, Option<States>> = IndexVec::filled(None, self.body.blocks.len());
        at[BlockId::new(0)] = Some(self.entry());
        let mut work = VecDeque::from([BlockId::new(0)]);
        while let Some(block) = work.pop_front() {
            let Some(mut states) = at[block].clone() else { continue };
            for next in self.block(block, &mut states) {
                let changed = match &mut at[next] {
                    Some(there) => there.join(&states),
                    slot @ None => {
                        *slot = Some(states.clone());
                        true
                    }
                };
                if changed {
                    work.push_back(next);
                }
            }
        }
        self.reporting = true;
        self.moved_out();
        for (block, states) in at.iter() {
            if let Some(states) = states {
                self.block(block, &mut states.clone());
            }
        }
        self.diag
    }

    /// Each linear value read whole out of a place reached through `.*` is moved out of a
    /// borrow or owner, which does not own it to give. The local it lands in is left out of the
    /// linear ones, so that the report is not repeated as a leak.
    fn moved_out(&mut self) {
        for (local, origin) in std::mem::take(&mut self.moved) {
            if self.linear.remove(&local) {
                let msg = Msg::new().text("this moves a linear value out of a borrow, which does not own it");
                let help = Msg::new()
                    .text("match or read through the borrow itself, without ")
                    .code(".*")
                    .text(", to bind by borrow");
                self.advise(errors::PLACE_MOVE_OUT_OF_BORROW, origin, msg, Some(help));
                self.derived(local);
            }
        }
    }

    /// Leaves out of the linear locals those holding `local` or a part of it, transitively.
    fn derived(&mut self, local: Local) {
        let (mut work, mut seen) = (vec![local], BTreeSet::from([local]));
        while let Some(from) = work.pop() {
            for (_, block) in self.body.blocks.iter() {
                for stmt in &block.stmts {
                    if let Rvalue::Use(Operand::Place(source)) = &stmt.value
                        && source.local == from
                        && seen.insert(stmt.place.local)
                    {
                        self.linear.remove(&stmt.place.local);
                        work.push(stmt.place.local);
                    }
                }
            }
        }
    }

    /// Parameters are `init`, other locals `uninit`; a borrow parameter's target is in the
    /// state its mode needs before the call.
    fn entry(&self) -> States {
        let mut locals = IndexVec::filled(State::Uninit, self.body.locals.len());
        for &p in &self.body.params {
            locals[p] = State::Init;
        }
        let targets = self.targets.iter().map(|(&p, &(before, _))| (p, before)).collect();
        States { locals, parts: BTreeMap::new(), targets }
    }

    /// Runs `block` on `states`, and gives the blocks control goes to next.
    fn block(&mut self, block: BlockId, states: &mut States) -> Vec<BlockId> {
        let b = &self.body.blocks[block];
        for stmt in &b.stmts {
            if stmt.value == Rvalue::Dead {
                self.scope_ends(stmt.place.local, states, stmt.origin);
                continue;
            }
            self.rvalue(&stmt.value, states, stmt.origin);
            self.write(&stmt.place, states, stmt.origin);
        }
        let origin = b.term.origin;
        match &b.term.kind {
            TerminatorKind::Return => {
                self.returns(states, origin);
                Vec::new()
            }
            TerminatorKind::Unreachable => Vec::new(),
            TerminatorKind::Goto(to) => vec![*to],
            TerminatorKind::Branch { cond, then, otherwise } => {
                self.operand(cond, states, origin);
                vec![*then, *otherwise]
            }
            TerminatorKind::Yield { value, resume } => {
                self.operand(value, states, origin);
                vec![*resume]
            }
            TerminatorKind::Defer { consumes, run, skip } => {
                self.deferred(consumes, *run, *skip, states, origin)
            }
        }
    }

    /// At a return, each borrow parameter's target that its mode leaves `init` must be, and
    /// every linear place must be `uninit`: nothing is closed implicitly.
    fn returns(&mut self, states: &States, origin: OriginId) {
        let leaked: Vec<Local> =
            self.linear.iter().copied().filter(|&l| states.locals[l] != State::Uninit).collect();
        for local in leaked {
            self.leak(local, origin, "returning");
        }
        let unwritten: Vec<Local> = self
            .targets
            .iter()
            .filter(|(local, (_, after))| {
                *after == State::Init && states.targets.get(local) != Some(&State::Init)
            })
            .map(|(&local, _)| local)
            .collect();
        for local in unwritten {
            let name = self.body.locals[local].name.map_or("", Symbol::as_str);
            let msg =
                Msg::new().code(name).text(" is not written on this path, and must be before returning");
            self.report(errors::PLACE_OUT_UNWRITTEN, origin, msg);
        }
    }

    /// At the end of the scope of `local`'s binding, a linear value it holds is leaked.
    fn scope_ends(&mut self, local: Local, states: &mut States, origin: OriginId) {
        if self.linear.contains(&local) && states.locals[local] != State::Uninit {
            self.leak(local, origin, "its scope ends");
        }
        states.end(local);
    }

    /// Reports the linear value in `local` as still held at `origin`, before `when`.
    fn leak(&mut self, local: Local, origin: OriginId, when: &str) {
        let what = self.body.locals[local]
            .name
            .map_or_else(|| Msg::new().text("a linear value"), |n| Msg::new().code(n.as_str()));
        let msg = what
            .text(" is still held here, and linear values must be consumed before ")
            .text(when.to_owned());
        let help = Msg::new()
            .text("pass it to what consumes it, such as a close or a free, or ")
            .code("defer")
            .text(" that call where the value is made");
        self.advise(errors::PLACE_LINEAR_LEAK, origin, msg, Some(help));
    }

    fn report(&mut self, id: DiagId, origin: OriginId, msg: Msg) {
        self.advise(id, origin, msg, None);
    }

    /// Reports, with `help` on what to do when given.
    fn advise(&mut self, id: DiagId, origin: OriginId, msg: Msg, help: Option<Msg>) {
        if self.reporting && self.reported.insert((id.clone(), origin)) {
            let mut d = Diagnostic::error(id, msg).at(origin, None);
            if let Some(help) = help {
                d = d.child(ChildKind::Help, help);
            }
            self.diag.error(d);
        }
    }
}
