use crate::{Action, Command};

/// A way a command tree leaves the declared vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Violation {
    /// A leaf command named outside the verb table, under a group that is not a verb's.
    NotAVerb { path: Vec<&'static str> },
    /// A name that two siblings answer to.
    Ambiguous { path: Vec<&'static str>, name: &'static str },
}

impl<Cx> Command<Cx> {
    /// Every way the tree below it, itself included, strays from the verb table; empty if none.
    #[must_use]
    pub fn verify(&self) -> Vec<Violation> {
        let mut found = Vec::new();
        self.verify_below(&mut Vec::new(), None, &mut found);
        found
    }

    fn verify_below(&self, path: &mut Vec<&'static str>, parent: Option<&Self>, found: &mut Vec<Violation>) {
        path.push(self.name);
        match (self.verb, &self.action) {
            (None, Action::Run(_)) if parent.is_some_and(|p| p.verb.is_none()) => {
                found.push(Violation::NotAVerb { path: path.clone() });
            }
            _ => {}
        }
        let children = self.children();
        for (at, child) in children.iter().enumerate() {
            let names = child.aliases.iter().copied().chain([child.name]);
            for name in names {
                if children[..at].iter().any(|before| before.answers(name)) {
                    found.push(Violation::Ambiguous { path: path.clone(), name });
                }
            }
            child.verify_below(path, Some(self), found);
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests;
