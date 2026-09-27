//! Completion scripts generated from the command tree: commands, options, and the values each
//! parameter declares (choices listed, paths completed by the shell).

mod bash;
mod fish;
mod nu;

use crate::{Param, Program, Selected, Values};

/// A shell a completion script is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Bash,
    Zsh,
    Fish,
    Nu,
}

impl Dialect {
    pub const ALL: [Self; 4] = [Self::Bash, Self::Zsh, Self::Fish, Self::Nu];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Nu => "nu",
        }
    }
}

/// The commands of a program, as completion scripts read them; each dialect's module writes
/// its script as a method.
pub(crate) struct Tree<'p> {
    program: &'static str,
    /// Every command, parents first.
    pub(crate) nodes: Vec<Node<'p>>,
}

/// One command of the tree, as a script completes it.
pub(crate) struct Node<'p> {
    /// The words selecting it below the program, names or aliases: `["pkg", "add"]`.
    pub(crate) path: Vec<&'static str>,
    about: &'static str,
    /// The commands below it, with what each does.
    children: Vec<(&'static str, &'static str)>,
    options: Vec<&'p Param>,
    positionals: Vec<&'p Param>,
}

impl<Cx> Program<Cx> {
    /// The completion script of this program for `dialect`.
    #[must_use]
    pub fn completion(&self, dialect: Dialect) -> String {
        let tree = self.tree();
        match dialect {
            Dialect::Bash => tree.bash(),
            Dialect::Zsh => tree.zsh(),
            Dialect::Fish => tree.fish(),
            Dialect::Nu => tree.nu(),
        }
    }

    /// Every command, once under each path of names and aliases selecting it.
    pub(crate) fn tree(&self) -> Tree<'_> {
        let mut nodes = Vec::new();
        let mut pending = vec![(Selected::root(&self.root), Vec::new())];
        while let Some((selected, path)) = pending.pop() {
            let command = selected.last();
            for child in command.children().iter().rev() {
                for &spelling in std::iter::once(&child.name).chain(child.aliases).rev() {
                    let below = path.iter().copied().chain([spelling]).collect();
                    pending.push((selected.then(child), below));
                }
            }
            nodes.push(Node {
                path,
                about: command.about,
                children: command.children().iter().map(|c| (c.name, c.about)).collect(),
                options: selected.options().collect(),
                positionals: command.positionals().collect(),
            });
        }
        Tree { program: self.root.name, nodes }
    }
}

impl Node<'_> {
    /// The words selecting it, joined by spaces.
    fn key(&self) -> String {
        self.path.join(" ")
    }

    /// The choices of the first positional, which is what a word after the command completes to.
    fn positional_choices(&self) -> Option<Vec<&'static str>> {
        self.positionals.first().and_then(|p| p.choices())
    }

    fn positional_paths(&self) -> bool {
        self.positionals.first().is_some_and(|p| matches!(p.values, Values::Path(_)))
    }
}
