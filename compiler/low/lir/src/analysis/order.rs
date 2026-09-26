//! The order blocks are emitted in: reverse postorder, with each loop's blocks kept together, so
//! that a value whose live range ends at a loop's exit is not live across unrelated blocks. A loop
//! is therefore a span of positions.

use vun_codec::{Decode, Encode};
use vun_index::IndexVec;

use crate::entity::Block;
use crate::func::Function;

use super::Loops;

pub struct Order {
    blocks: Vec<Block>,
    position: IndexVec<Block, Option<u32>>,
    /// For a loop's header, the position of the loop's last block.
    end: IndexVec<Block, u32>,
}

/// What a loop holds, in reverse postorder: its own blocks and the loops inside it.
#[derive(Clone, Copy, Encode, Decode)]
enum Item {
    Block(Block),
    Loop(Block),
}

impl Order {
    #[must_use]
    pub fn of(func: &Function, loops: &Loops) -> Self {
        let count = func.blocks.len();
        let mut root = Vec::new();
        let mut inside: IndexVec<Block, Vec<Item>> = IndexVec::filled(Vec::new(), count);
        for &block in loops.rpo() {
            let (owner, item) = if loops.is_header(block) {
                inside[block].push(Item::Block(block));
                (loops.parent(block), Item::Loop(block))
            } else {
                (loops.innermost(block), Item::Block(block))
            };
            match owner {
                Some(header) => inside[header].push(item),
                None => root.push(item),
            }
        }
        let mut order = Self {
            blocks: Vec::with_capacity(count),
            position: IndexVec::filled(None, count),
            end: IndexVec::filled(0, count),
        };
        order.place(&root, &inside);
        order
    }

    fn place(&mut self, items: &[Item], inside: &IndexVec<Block, Vec<Item>>) {
        for &item in items {
            match item {
                Item::Block(block) => {
                    self.position[block] = Some(u32::try_from(self.blocks.len()).expect("blocks fit u32"));
                    self.blocks.push(block);
                }
                Item::Loop(header) => {
                    self.place(&inside[header], inside);
                    self.end[header] = u32::try_from(self.blocks.len() - 1).expect("blocks fit u32");
                }
            }
        }
    }

    /// The reached blocks, in the order they are emitted.
    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Where `block` is emitted; `None` for a block the entry does not reach.
    #[must_use]
    pub fn position(&self, block: Block) -> Option<u32> {
        self.position[block]
    }

    /// The first and last positions of the loop headed by `header`.
    #[must_use]
    pub fn span(&self, header: Block) -> (u32, u32) {
        (self.position[header].unwrap_or(0), self.end[header])
    }

    /// The block emitted after `block`, where it falls through without a jump.
    #[must_use]
    pub fn next(&self, block: Block) -> Option<Block> {
        let at = self.position(block)?;
        self.blocks.get(at as usize + 1).copied()
    }
}
