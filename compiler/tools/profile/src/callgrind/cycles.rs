//! Functions that call each other, which no call tree has one inclusive cost for.

/// The strongly connected components of a call graph: functions that reach each other.
pub(super) struct Cycles {
    /// The component of each function.
    component: Vec<usize>,
    pub(super) count: usize,
    /// How many functions each component holds.
    sizes: Vec<usize>,
}

/// A function in Tarjan's walk: where it is on the stack, and how far back it reaches.
#[derive(Clone, Copy)]
struct Visit {
    index: usize,
    low: usize,
    on_stack: bool,
}

impl Cycles {
    /// The components of `functions` functions with these calls, `(caller, callee)`.
    pub(super) fn of(functions: usize, calls: impl Iterator<Item = (usize, usize)>) -> Self {
        let mut callees = vec![Vec::new(); functions];
        for (caller, callee) in calls {
            callees[caller].push(callee);
        }
        let mut visits: Vec<Option<Visit>> = vec![None; functions];
        let (mut component, mut count, mut next) = (vec![usize::MAX; functions], 0, 0);
        let mut stack = Vec::new();
        for root in 0..functions {
            if visits[root].is_some() {
                continue;
            }
            // The walk's own stack: a function and how many of its callees were visited.
            let mut walk = vec![(root, 0)];
            visits[root] = Some(Visit { index: next, low: next, on_stack: true });
            next += 1;
            stack.push(root);
            while let Some(&(at, seen)) = walk.last() {
                if let Some(&callee) = callees[at].get(seen) {
                    walk.last_mut().expect("the walk holds `at`").1 += 1;
                    match visits[callee] {
                        None => {
                            visits[callee] = Some(Visit { index: next, low: next, on_stack: true });
                            next += 1;
                            stack.push(callee);
                            walk.push((callee, 0));
                        }
                        Some(Visit { index, on_stack: true, .. }) => {
                            let visit = visits[at].as_mut().expect("visited");
                            visit.low = visit.low.min(index);
                        }
                        Some(_) => {}
                    }
                    continue;
                }
                walk.pop();
                let Visit { index, low, .. } = visits[at].expect("visited");
                if let Some(&(caller, _)) = walk.last() {
                    let parent = visits[caller].as_mut().expect("visited");
                    parent.low = parent.low.min(low);
                }
                if low == index {
                    while let Some(member) = stack.pop() {
                        visits[member].as_mut().expect("visited").on_stack = false;
                        component[member] = count;
                        if member == at {
                            break;
                        }
                    }
                    count += 1;
                }
            }
        }
        let mut sizes = vec![0; count];
        for &c in &component {
            sizes[c] += 1;
        }
        Self { component, count, sizes }
    }

    pub(super) fn of_function(&self, function: usize) -> usize {
        self.component[function]
    }

    /// Whether `function` calls itself through others: its component holds more than it.
    pub(super) fn is_cyclic(&self, function: usize) -> bool {
        self.sizes[self.component[function]] > 1
    }

    /// Whether `a` and `b` call each other, directly or through others.
    pub(super) fn together(&self, a: usize, b: usize) -> bool {
        self.component[a] == self.component[b]
    }
}
