//! Splitting token lists at their commas, with the nesting of angle brackets, which are
//! punctuation rather than groups.

use proc_macro::{Spacing, TokenTree};

/// The nesting of angle brackets; the `>` of `->` does not close one.
#[derive(Default)]
pub(crate) struct Depth {
    level: usize,
    arrow: bool,
}

impl Depth {
    /// The trees up to the `>` closing an angle bracket already opened.
    pub(crate) fn angled(trees: &mut impl Iterator<Item = TokenTree>) -> Vec<TokenTree> {
        let mut depth = Self::default();
        trees.take_while(|tree| depth.step(tree)).collect()
    }

    /// `trees` split at their commas, those inside angle brackets aside when `angles`; empty
    /// segments, as after a trailing comma, are dropped.
    pub(crate) fn split(trees: Vec<TokenTree>, angles: bool) -> Vec<Vec<TokenTree>> {
        let mut depth = Self::default();
        let mut segments = vec![Vec::new()];
        for tree in trees {
            if angles {
                depth.step(&tree);
            }
            if depth.level == 0 && tree.is(',') {
                segments.push(Vec::new());
            } else {
                segments.last_mut().expect("one segment at least").push(tree);
            }
        }
        segments.retain(|segment| !segment.is_empty());
        segments
    }

    /// Steps over `tree`, returning whether it is inside the brackets it started in.
    fn step(&mut self, tree: &TokenTree) -> bool {
        let arrow = std::mem::take(&mut self.arrow);
        let TokenTree::Punct(punct) = tree else { return true };
        match punct.as_char() {
            '<' => self.level += 1,
            '>' if !arrow => match self.level.checked_sub(1) {
                Some(level) => self.level = level,
                None => return false,
            },
            '-' => self.arrow = punct.spacing() == Spacing::Joint,
            _ => {}
        }
        true
    }
}

/// Questions about one token.
pub(crate) trait Tree {
    fn is(&self, c: char) -> bool;
    fn is_word(&self, word: &str) -> bool;
}

impl Tree for TokenTree {
    fn is(&self, c: char) -> bool {
        matches!(self, Self::Punct(p) if p.as_char() == c)
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self, Self::Ident(i) if i.to_string() == word)
    }
}
