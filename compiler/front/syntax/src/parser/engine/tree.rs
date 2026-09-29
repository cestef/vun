//! The nodes read, kept until the root is complete and then written into the tree in postorder,
//! each as the first kind it spells that the position holding it holds.

use std::ops::Range;

use vun_grammar::{KindSet, NodeKind, TRANSPARENT};

use super::spec::Spec;
use crate::cst::{Anchor, Builder};
use crate::tokens::{TokenId, Tokens};

/// A node read: the kinds it spells, preferred first, its anchor and its pieces, as ranges of the
/// tree's stores.
struct Built {
    spellings: Range<usize>,
    anchor: Anchor,
    pieces: Range<usize>,
}

/// A kind a node spells, and where what the positions holding its children then take begins in
/// the tree's store, one for each child. An `Error` takes none: its children stand as they are.
#[derive(Clone, Copy)]
struct Spelling {
    kind: NodeKind,
    specs: usize,
}

/// What a node holds, in source order.
#[derive(Clone, Copy, Debug)]
pub(in crate::parser) enum Piece {
    /// A token its production reads.
    Token(TokenId),
    /// A token its production reads, absent before this one.
    Absent(TokenId),
    /// A token no production reads.
    Skipped(TokenId),
    Child(u32),
}

/// The nodes read so far, what they spell and hold.
#[derive(Default)]
pub(in crate::parser) struct Tree {
    nodes: Vec<Built>,
    spellings: Vec<Spelling>,
    specs: Vec<Spec>,
    pieces: Vec<Piece>,
}

impl Tree {
    /// Spells the next node added as `kind`, whose children the positions holding them take as
    /// `specs` say, in order.
    pub(super) fn spell(&mut self, kind: NodeKind, specs: impl IntoIterator<Item = Spec>) {
        self.spellings.push(Spelling { kind, specs: self.specs.len() });
        self.specs.extend(specs);
    }

    /// Adds the node spelled since the last one, anchored at `anchor` and holding `pieces`.
    pub(super) fn add(&mut self, anchor: Anchor, pieces: impl IntoIterator<Item = Piece>) -> u32 {
        let spelled = self.nodes.last().map_or(0, |n| n.spellings.end);
        let start = self.pieces.len();
        self.pieces.extend(pieces);
        let node =
            Built { spellings: spelled..self.spellings.len(), anchor, pieces: start..self.pieces.len() };
        self.nodes.push(node);
        u32::try_from(self.nodes.len() - 1).expect("fewer nodes than tokens")
    }

    /// Adds a node of `kind` holding nothing.
    pub(super) fn leaf(&mut self, kind: NodeKind, anchor: Anchor) -> u32 {
        self.spell(kind, []);
        self.add(anchor, [])
    }

    fn node(&self, n: u32) -> &Built {
        &self.nodes[n as usize]
    }

    fn spellings(&self, n: u32) -> &[Spelling] {
        &self.spellings[self.node(n).spellings.clone()]
    }

    fn pieces(&self, n: u32) -> &[Piece] {
        &self.pieces[self.node(n).pieces.clone()]
    }

    /// The kinds of the nodes `n` ends with: its own, and for a transparent one, those of the
    /// nodes its last child ends with.
    pub(super) fn ends(&self, n: u32, tokens: &Tokens) -> KindSet {
        self.spellings(n).iter().fold(KindSet::EMPTY, |set, s| match self.last_child(n) {
            Some(last) if self.transparent(n, s.kind, tokens) => set.union(self.ends(last, tokens)),
            _ => set.with(s.kind),
        })
    }

    /// The first token of `n`, which its anchor or its first piece holds.
    pub(super) fn first_token(&self, n: u32) -> TokenId {
        let anchor = self.node(n).anchor.token();
        let first = self.pieces(n).first().map(|piece| match *piece {
            Piece::Token(t) | Piece::Absent(t) | Piece::Skipped(t) => t,
            Piece::Child(c) => self.first_token(c),
        });
        first.map_or(anchor, |t| t.min(anchor))
    }

    fn last_child(&self, n: u32) -> Option<u32> {
        self.pieces(n).iter().rev().find_map(|p| if let Piece::Child(c) = p { Some(*c) } else { None })
    }

    /// Whether `n` spelled as `kind` is transparent: a kind `transparent` lists, written with a
    /// token making it so.
    fn transparent(&self, n: u32, kind: NodeKind, tokens: &Tokens) -> bool {
        let Anchor::Token(anchor) = self.node(n).anchor else { return false };
        TRANSPARENT.iter().any(|row| row.kind == kind && row.tokens.contains(&tokens.kind(anchor)))
    }

    /// Whether `n` may stand where `spec` says, as one of the kinds it spells.
    pub(super) fn fits(&self, n: u32, spec: &Spec, tokens: &Tokens) -> bool {
        self.spellings(n).iter().any(|s| self.fits_as(n, s.kind, spec, tokens))
    }

    /// Whether `n` spelled as `kind` may stand where `spec` says: as it is, or where a transparent
    /// node may stand for what it ends with, as its last child. An `Error` stands anywhere.
    fn fits_as(&self, n: u32, kind: NodeKind, spec: &Spec, tokens: &Tokens) -> bool {
        let starts = || spec.starting.is_none_or(|s| s.contains(tokens.kind(self.first_token(n))));
        let through = || {
            spec.through
                && self.transparent(n, kind, tokens)
                && self.last_child(n).is_some_and(|c| self.fits(c, spec, tokens))
        };
        kind == NodeKind::Error || (starts() && (spec.holds(kind) || through()))
    }

    /// Writes `n` and what it holds into `out`, as the first kind it spells that `taking` holds;
    /// the last child of a transparent node standing there for what it ends with takes what the
    /// node's position takes.
    pub(super) fn write(&self, n: u32, taking: Option<&Spec>, out: &mut Builder, tokens: &Tokens) {
        let spellings = self.spellings(n);
        let fits = |s: &&Spelling| taking.is_none_or(|spec| self.fits_as(n, s.kind, spec, tokens));
        let spelling = spellings.iter().find(fits).or(spellings.first()).copied();
        let spelling = spelling.unwrap_or(Spelling { kind: NodeKind::Error, specs: 0 });
        // Where the position takes the node only as what it ends with, so does its last child.
        let through = taking.is_some_and(|spec| !spec.holds(spelling.kind));
        let last = self.last_child(n).filter(|_| through && self.transparent(n, spelling.kind, tokens));
        let (mark, mut specs) = (out.mark(), self.specs.get(spelling.specs..).unwrap_or_default().iter());
        for piece in self.pieces(n) {
            match *piece {
                Piece::Token(t) => out.node(NodeKind::Punct, Anchor::Token(t), out.mark()),
                Piece::Absent(t) => out.node(NodeKind::Punct, Anchor::Missing(t), out.mark()),
                Piece::Skipped(t) => out.node(NodeKind::Skipped, Anchor::Token(t), out.mark()),
                Piece::Child(c) => {
                    let own = if spelling.kind == NodeKind::Error { None } else { specs.next() };
                    self.write(c, if last == Some(c) { taking } else { own }, out, tokens);
                }
            }
        }
        out.node(spelling.kind, self.node(n).anchor, mark);
    }
}
