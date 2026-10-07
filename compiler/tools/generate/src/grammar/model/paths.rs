//! Paths naming the grammar's declarations: through an alias of a file's namespace
//! (`grammar := lang.syntax.grammar`), a namespace, or in any file, a declaration or a list of a
//! kind, and what they hold.

use vun_diag::Msg;
use vun_syntax::literal::{Decl, FromLiteral, Literal, Path};

use super::resolve::{mismatch, unresolved};
use super::written::Entry;
use crate::generate::{Read, Source};
use crate::grammar::{GENERATOR, KIND, SHAPE};

/// The grammar's files, by their namespaces, which paths name declarations through.
pub(super) struct Files<'d, 's> {
    files: Vec<(Vec<String>, &'d [Decl<'s>])>,
}

/// What a path names: a table's value, or the entries of a kind or of a list of one.
pub(super) enum Target<'d, 's> {
    Table(&'d Literal<'s>),
    Entries(Vec<Entry>),
}

impl<'d, 's> Files<'d, 's> {
    pub(super) fn of(sources: &'d [Source<'s>]) -> Self {
        Self { files: sources.iter().map(|s| (s.namespace().unwrap_or_default(), &s.decls[..])).collect() }
    }

    /// The declarations of every file, read as one record.
    pub(super) fn read<T: FromLiteral>(&self) -> Read<T> {
        let decls: Vec<Decl<'_>> = self.files.iter().flat_map(|(_, d)| d.iter().cloned()).collect();
        T::from_decls(&decls).map_err(|m| mismatch("the grammar", &m))
    }

    /// The namespace a declaration aliases, if it names a file of the grammar: `grammar :=
    /// lang.syntax.grammar`.
    fn alias(&self, name: &str) -> Option<&[String]> {
        let decl = self.files.iter().flat_map(|(_, d)| d.iter()).find(|d| d.name == name)?;
        let Literal::Path(segments) = &decl.value else { return None };
        let (namespace, _) = self.files.iter().find(|(ns, _)| ns.iter().eq(segments.iter()))?;
        Some(namespace)
    }

    /// What `path` names: through an alias or a file's namespace, or in any file, a declaration
    /// or a list of a kind.
    pub(super) fn target(&self, path: &Path) -> Read<Target<'d, 's>> {
        let missing = || unresolved(&path.name(), "no declaration of the grammar is at ", &path.0.join("."));
        let (first, rest) = path.0.split_first().ok_or_else(missing)?;
        let expanded: Vec<String> = match self.alias(first) {
            Some(namespace) => namespace.iter().chain(rest).cloned().collect(),
            None => path.0.clone(),
        };
        let (scope, inner): (Vec<&[Decl<'s>]>, &[String]) =
            match self.files.iter().find(|(ns, _)| !ns.is_empty() && expanded.starts_with(ns)) {
                Some((ns, decls)) => (vec![*decls], &expanded[ns.len()..]),
                None => (self.files.iter().map(|(_, d)| *d).collect(), &expanded[..]),
            };
        let find = |name: &str| scope.iter().flat_map(|d| d.iter()).find(|d| d.name == name);
        let entries =
            |value: &Literal<'_>| value.read::<Vec<Entry>>().map_err(|m| mismatch(&path.name(), &m));
        match inner {
            [name] => {
                let decl = find(name).ok_or_else(missing)?;
                match &decl.value {
                    Literal::Record(lists) if decl.marked(KIND) => {
                        let mut all = Vec::new();
                        for list in lists {
                            all.extend(entries(&list.value)?);
                        }
                        Ok(Target::Entries(all))
                    }
                    value => Ok(Target::Table(value)),
                }
            }
            [kind, list] => {
                let list = find(kind).and_then(|d| d.value.get(list)).ok_or_else(missing)?;
                Ok(Target::Entries(entries(list)?))
            }
            _ => Err(missing()),
        }
    }

    /// The names the path's target holds: a table's `.name`s at any depth, or its entries'.
    pub(super) fn names(&self, path: &Path) -> Read<Vec<String>> {
        Ok(match self.target(path)? {
            Target::Table(value) => {
                let mut names = Vec::new();
                mentioned(value, &mut names);
                names
            }
            Target::Entries(entries) => entries.into_iter().map(|e| e.name).collect(),
        })
    }

    /// The spellings of the entries the path names.
    pub(super) fn words(&self, path: &Path) -> Read<Vec<String>> {
        let unspelled =
            || GENERATOR.error(SHAPE, &path.name(), Msg::new().text("names a table, or an unspelled entry"));
        match self.target(path)? {
            Target::Entries(entries) => entries.into_iter().map(|e| e.text.ok_or_else(unspelled)).collect(),
            Target::Table(_) => Err(unspelled()),
        }
    }
}

/// Every `.name` a value holds, at any depth.
fn mentioned(value: &Literal<'_>, into: &mut Vec<String>) {
    match value {
        Literal::Name(name) => into.push((*name).to_owned()),
        Literal::List(items) => items.iter().for_each(|v| mentioned(v, into)),
        Literal::Record(fields) => fields.iter().for_each(|f| mentioned(&f.value, into)),
        _ => {}
    }
}

/// The name a path ends with, which generated code refers to its declaration by.
pub(super) trait Named {
    fn name(&self) -> String;
}

impl Named for Path {
    fn name(&self) -> String {
        self.0.last().cloned().unwrap_or_default()
    }
}
