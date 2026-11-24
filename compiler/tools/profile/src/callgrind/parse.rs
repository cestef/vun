//! Reading the file callgrind writes. The format names each object, file and function once, as
//! `(id) name`, and refers to it as `(id)` after; a cost line is its position, then one number
//! per event; the line after `calls=` holds what that call cost in all.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use vun_intern::Symbol;

use super::fold::Node;
use super::{Profile, Source};

/// The event whose cost is read.
const EVENT: &str = "Ir";

/// What the file lacks or holds wrongly, at a line.
#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub reason: Reason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// `(id)` refers to a name that was never given.
    Unnamed,
    /// A cost line without a number where the event is.
    Cost,
    /// The file does not count the event.
    Event,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self.reason {
            Reason::Unnamed => "refers to a name that was not given before",
            Reason::Cost => "has no cost where the instructions are",
            Reason::Event => "the profile does not count instructions",
        };
        write!(f, "line {} of the callgrind file {said}", self.line)
    }
}

impl Error for ParseError {}

/// The names of one kind, by the id the file gave them.
#[derive(Default)]
struct Names(HashMap<u32, Symbol>);

impl Names {
    /// The name `spec` (`(id) name` or `(id)`) stands for, remembered when it gives one.
    fn resolve(&mut self, spec: &str) -> Option<Symbol> {
        let (id, name) = spec.strip_prefix('(')?.split_once(')')?;
        let id: u32 = id.parse().ok()?;
        let name = name.trim();
        if name.is_empty() {
            return self.0.get(&id).copied();
        }
        let symbol = Symbol::intern(name);
        self.0.insert(id, symbol);
        Some(symbol)
    }
}

/// What the file says about its columns: where the cost of [`EVENT`] is in a cost line.
struct Columns {
    positions: usize,
    event: usize,
}

/// Where code runs: the object, the file its lines are in, and the function.
#[derive(Clone, Copy, Default)]
struct Place {
    object: Option<Symbol>,
    file: Option<Symbol>,
    function: Option<usize>,
}

#[derive(Default)]
struct Reader {
    objects: Names,
    files: Names,
    functions: Names,
    sources: HashMap<(Option<Symbol>, Option<Symbol>), Source>,
    index: HashMap<Symbol, usize>,
    found: Vec<Node>,
    /// Instructions by the source of the line that ran.
    spent: HashMap<Source, u64>,
    /// By caller and callee: how often the call ran, and what it cost.
    edges: HashMap<(usize, usize), (u64, u64)>,
    here: Place,
    /// The file `fn=` put the code in, which `fe=` returns to.
    home: Option<Symbol>,
    /// The callee and call count a `calls=` line announced, whose cost the next line holds.
    calling: Option<(usize, u64)>,
    callee: Place,
}

impl Reader {
    fn source(&mut self, file: Option<Symbol>, object: Option<Symbol>) -> Source {
        *self
            .sources
            .entry((file, object))
            .or_insert_with(|| Source::of(file.map_or("", Symbol::as_str), object.map_or("", Symbol::as_str)))
    }

    /// The accumulator of the function `name` in `place`, made when it is first seen. Recursion
    /// levels (`name'2`) are functions of their own here.
    fn function(&mut self, name: Symbol, place: Place) -> usize {
        if let Some(&at) = self.index.get(&name) {
            return at;
        }
        let source = self.source(place.file, place.object);
        self.found.push(Node { name, source, own: 0, calls: 0 });
        self.index.insert(name, self.found.len() - 1);
        self.found.len() - 1
    }

    fn line(&mut self, text: &str, columns: &Columns) -> Option<()> {
        if let Some((key, value)) = text.split_once('=') {
            return self.directive(key, value);
        }
        let cost: u64 = text.split_whitespace().nth(columns.positions + columns.event)?.parse().ok()?;
        if let Some((callee, calls)) = self.calling.take() {
            if let Some(caller) = self.here.function.filter(|&caller| caller != callee) {
                let edge = self.edges.entry((caller, callee)).or_default();
                edge.0 += calls;
                edge.1 += cost;
            }
            self.found[callee].calls += calls;
        } else {
            let caller = self.here.function?;
            self.found[caller].own += cost;
            let source = self.source(self.here.file, self.here.object);
            *self.spent.entry(source).or_default() += cost;
        }
        Some(())
    }

    fn directive(&mut self, key: &str, value: &str) -> Option<()> {
        match key {
            "ob" => self.here.object = Some(self.objects.resolve(value)?),
            "fl" => {
                let file = self.files.resolve(value)?;
                (self.here.file, self.home) = (Some(file), Some(file));
            }
            "fi" | "fe" => self.here.file = Some(self.files.resolve(value).or(self.home)?),
            "fn" => {
                let name = self.functions.resolve(value)?;
                self.here.file = self.home;
                self.here.function = Some(self.function(name, self.here));
            }
            "cob" => self.callee.object = Some(self.objects.resolve(value)?),
            "cfi" | "cfl" => self.callee.file = Some(self.files.resolve(value)?),
            "cfn" => {
                let name = self.functions.resolve(value)?;
                let place = Place {
                    object: self.callee.object.or(self.here.object),
                    file: self.callee.file.or(self.here.file),
                    function: None,
                };
                self.callee.function = Some(self.function(name, place));
                (self.callee.object, self.callee.file) = (None, None);
            }
            "calls" => {
                let calls = value.split_whitespace().next()?.parse().ok()?;
                self.calling = Some((self.callee.function.take()?, calls));
            }
            _ => {}
        }
        Some(())
    }
}

impl FromStr for Profile {
    type Err = ParseError;

    /// The profile `text` holds, in callgrind's format, counting instructions.
    fn from_str(text: &str) -> Result<Self, ParseError> {
        let mut columns = Columns { positions: 1, event: 0 };
        let mut counted = false;
        let mut reader = Reader::default();
        for (number, line) in text.lines().enumerate() {
            let at = |reason| ParseError { line: number + 1, reason };
            if let Some(events) = line.strip_prefix("events:") {
                columns.event =
                    events.split_whitespace().position(|e| e == EVENT).ok_or(at(Reason::Event))?;
                counted = true;
            } else if let Some(positions) = line.strip_prefix("positions:") {
                columns.positions = positions.split_whitespace().count();
            } else if line.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '*'))
                || line.contains('=')
            {
                let reason = if line.contains('=') { Reason::Unnamed } else { Reason::Cost };
                reader.line(line, &columns).ok_or(at(reason))?;
            }
        }
        if counted {
            Ok(Self::folded(&reader.found, &reader.edges, reader.spent))
        } else {
            Err(ParseError { line: 0, reason: Reason::Event })
        }
    }
}
