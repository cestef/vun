//! What a covered run reached of the code a program holds (spec 11, section 6): the blocks of each
//! function reached, with the count each makes and where its code is written, read against the
//! counts the run wrote. A line is hit as often as the most-run block holding code written on it,
//! a function as often as its first block ran, which is how often it was called. Blocks whose
//! numbers meet modulo `lang.cover.size` share a count, so a block that never ran may be counted
//! with one that did; [`Reach::shared`] says how many blocks do.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_mir::{Program, Region};
use vun_origin::{OriginTable, Resolve};
use vun_query::Cx;
use vun_sema_ty::call::Instantiation;
use vun_span::{LineCol, SourceMap};

use super::rows::{FileRow, FunctionRow};
use crate::Session;
use crate::testing::Covered;

/// Where a function begins in its file: line and column, counted from 1.
pub(super) type Start = (u32, u32);

/// A function of a file, and how many times it was called: instantiations of one function
/// literal add up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Function {
    /// The path of its declaration, where it has one.
    pub name: Option<String>,
    pub calls: u64,
}

/// What a run reached of a file: how often each line holding code ran, and each function was
/// called.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub lines: BTreeMap<u32, u64>,
    pub functions: BTreeMap<Start, Function>,
}

/// What a run reached of the files below a scope, by the path each is shown as.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    pub files: BTreeMap<PathBuf, Source>,
    /// How many blocks of the program share their count with another.
    pub shared: usize,
}

impl Session {
    /// What the run that wrote `covered` reached of the code below `scope`: the functions
    /// `program`, built covered, holds, and the functions of `files` it does not, which no run
    /// reached.
    pub fn reached(
        &mut self,
        program: &Program,
        covered: &Covered,
        scope: &Path,
        files: &[PathBuf],
    ) -> Reach {
        let below = std::path::absolute(scope).unwrap_or_else(|_| scope.to_path_buf());
        let mut reach = Reach::default();
        let mut seen = HashSet::new();
        let mut owners: HashMap<usize, usize> = HashMap::new();
        for i in program.instantiations() {
            seen.insert(i.clone());
            let Some(regions) = Region::of(&Cx::new(&self.db), i) else { continue };
            for region in &regions {
                *owners.entry(region.flag).or_default() += 1;
            }
            self.record(&mut reach, &below, (i, &regions), &|region| u64::from(covered.count(region.flag)));
        }
        reach.shared = owners.values().filter(|&&blocks| blocks > 1).sum();
        let sources = files.iter().filter(|path| path.file_name().is_some_and(|name| name != MANIFEST));
        let loaded: Vec<_> = sources.filter_map(|path| self.load(path)).collect();
        for file in loaded {
            for f in self.functions(file) {
                let i = Instantiation::plain(f);
                if !seen.contains(&i)
                    && let Some(regions) = Region::of(&Cx::new(&self.db), &i)
                {
                    self.record(&mut reach, &below, (&i, &regions), &|_| 0);
                }
            }
        }
        reach
    }

    /// Adds the lines and the function of the body of `i`, whose blocks are `regions`, written
    /// below `scope` to `reach`, a region having run `ran` says how many times.
    fn record(
        &self,
        reach: &mut Reach,
        scope: &Path,
        (i, regions): (&Instantiation, &[Region]),
        ran: &dyn Fn(&Region) -> u64,
    ) {
        let holds = |path: &str| std::path::absolute(path).is_ok_and(|path| path.starts_with(scope));
        let start = self.sources.locate(self.source_span(i.f.literal.expr.origin));
        if holds(start.path) {
            let LineCol { line, col, .. } = start.at;
            let calls: u64 = regions.iter().filter(|region| region.entry).map(ran).sum();
            let name = OriginTable::global().declaration_name(i.f.literal.expr.origin);
            let function = reach.source(start.path).functions.entry((line, col)).or_default();
            function.calls += calls;
            function.name = function.name.take().or(name.map(|n| n.as_str().to_owned()));
        }
        for region in regions {
            let count = ran(region);
            for &origin in &region.origins {
                let at = self.sources.locate(self.source_span(origin));
                if holds(at.path) {
                    let hits = reach.source(at.path).lines.entry(at.at.line).or_default();
                    *hits = (*hits).max(count);
                }
            }
        }
    }
}

impl Source {
    /// How many lines ran.
    #[must_use]
    pub fn lines_hit(&self) -> usize {
        self.lines.values().filter(|&&count| count > 0).count()
    }

    /// How many functions were called.
    #[must_use]
    pub fn functions_called(&self) -> usize {
        self.functions.values().filter(|f| f.calls > 0).count()
    }
}

impl Reach {
    fn source(&mut self, path: &str) -> &mut Source {
        self.files.entry(PathBuf::from(path)).or_default()
    }

    /// One row per file: the lines and the functions it reached, of those it holds.
    #[must_use]
    pub fn file_rows(&self) -> Vec<FileRow> {
        self.files
            .iter()
            .map(|(path, source)| FileRow {
                path: path.display().to_string(),
                lines: source.lines_hit(),
                of_lines: source.lines.len(),
                functions: source.functions_called(),
                of_functions: source.functions.len(),
            })
            .collect()
    }

    /// One row per function, the files in order, then the functions in the order they are written.
    #[must_use]
    pub fn function_rows(&self) -> Vec<FunctionRow> {
        self.files
            .iter()
            .flat_map(|(path, source)| {
                source.functions.iter().map(move |(&(line, column), function)| FunctionRow {
                    path: path.display().to_string(),
                    line,
                    column,
                    name: function.name.clone(),
                    calls: function.calls,
                })
            })
            .collect()
    }
}
