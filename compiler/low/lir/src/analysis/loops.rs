//! The loop forest (Wei, Mao, Zou and Chen, 2007): one depth-first walk tags each block with the
//! header of the innermost loop around it, irreducible loops included. The function is the root
//! loop, which has no header. The same walk gives the reverse postorder.

use vun_index::IndexVec;

use crate::entity::Block;
use crate::func::Function;

use super::Cfg;

pub struct Loops {
    /// The header of the innermost loop around each block; a header's is the loop around its own
    /// loop. `None` is the function's loop.
    header: IndexVec<Block, Option<Block>>,
    is_header: IndexVec<Block, bool>,
    /// Headers of loops entered other than through their header.
    irreducible: IndexVec<Block, bool>,
    /// The blocks the entry reaches, in reverse postorder.
    rpo: Vec<Block>,
}

/// The state of the walk.
struct Walk<'c> {
    cfg: &'c Cfg,
    loops: Loops,
    /// The depth of each block on the current path from the entry, 0 when not on it.
    depth: IndexVec<Block, u32>,
    visited: IndexVec<Block, bool>,
}

impl Loops {
    #[must_use]
    pub fn of(func: &Function, cfg: &Cfg) -> Self {
        let count = func.blocks.len();
        let loops = Self {
            header: IndexVec::filled(None, count),
            is_header: IndexVec::filled(false, count),
            irreducible: IndexVec::filled(false, count),
            rpo: Vec::new(),
        };
        let mut walk =
            Walk { cfg, loops, depth: IndexVec::filled(0, count), visited: IndexVec::filled(false, count) };
        if let Some(entry) = func.entry() {
            walk.run(entry);
        }
        let mut loops = walk.loops;
        loops.rpo.reverse();
        loops
    }

    /// The header of the innermost loop holding `block`: itself for a header.
    #[must_use]
    pub fn innermost(&self, block: Block) -> Option<Block> {
        if self.is_header[block] { Some(block) } else { self.header[block] }
    }

    /// The header of the loop around the loop headed by `header`.
    #[must_use]
    pub fn parent(&self, header: Block) -> Option<Block> {
        self.header[header]
    }

    #[must_use]
    pub fn is_header(&self, block: Block) -> bool {
        self.is_header[block]
    }

    #[must_use]
    pub fn is_irreducible(&self, header: Block) -> bool {
        self.irreducible[header]
    }

    /// The blocks the entry reaches, in reverse postorder.
    #[must_use]
    pub fn rpo(&self) -> &[Block] {
        &self.rpo
    }
}

impl Walk<'_> {
    fn run(&mut self, entry: Block) {
        let mut stack = vec![(entry, 0)];
        self.enter(entry, 1);
        while let Some(&(block, next)) = stack.last() {
            let succs = &self.cfg.succs[block];
            // Successors are walked last to first, so reverse postorder lists them in order.
            if next < succs.len() {
                let succ = succs[succs.len() - 1 - next];
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                if self.visited[succ] {
                    self.visited_again(block, succ);
                } else {
                    self.enter(succ, self.depth[block] + 1);
                    stack.push((succ, 0));
                }
                continue;
            }
            stack.pop();
            self.depth[block] = 0;
            self.loops.rpo.push(block);
            if let (Some(&(parent, _)), Some(header)) = (stack.last(), self.loops.header[block]) {
                self.tag(parent, header);
            }
        }
    }

    fn enter(&mut self, block: Block, depth: u32) {
        self.visited[block] = true;
        self.depth[block] = depth;
    }

    /// An edge from `from` to `to`, already visited: a back edge, an edge into a loop found
    /// earlier, or a re-entry making that loop irreducible.
    fn visited_again(&mut self, from: Block, to: Block) {
        if self.depth[to] > 0 {
            return self.tag(from, to);
        }
        let Some(mut header) = self.loops.header[to] else { return };
        loop {
            if self.depth[header] > 0 {
                return self.tag(from, header);
            }
            self.loops.irreducible[header] = true;
            match self.loops.header[header] {
                Some(outer) => header = outer,
                None => return,
            }
        }
    }

    /// Puts `block` in the loop headed by `header`, weaving it into the chain of loops found so
    /// far, ordered by their headers' depth on the path.
    fn tag(&mut self, block: Block, header: Block) {
        if block == header {
            // A block jumping to itself is a loop of its own.
            self.loops.is_header[header] = true;
            return;
        }
        let (mut inner, mut outer) = (block, header);
        while let Some(current) = self.loops.header[inner] {
            if current == outer {
                return;
            }
            if self.depth[current] < self.depth[outer] {
                self.set(inner, outer);
                inner = outer;
                outer = current;
            } else {
                inner = current;
            }
        }
        self.set(inner, outer);
    }

    fn set(&mut self, block: Block, header: Block) {
        self.loops.header[block] = Some(header);
        self.loops.is_header[header] = true;
    }
}
