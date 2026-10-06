//! What a covered run reached of the code a program holds (spec 11, section 6): the blocks of each
//! function reached, with the flag each sets and where its code is written, read against the flags
//! the run wrote. A line is reached when a block holding code written on it ran, a function when
//! its first block did. Blocks whose numbers meet modulo `lang.cover.size` share a flag, so a
//! block that never ran may be counted with one that did.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use vun_build::layout::MANIFEST;
use vun_data::Object;
use vun_mir::{Program, Region};
use vun_origin::Resolve;
use vun_query::Cx;
use vun_sema_ty::call::Instantiation;
use vun_span::{LineCol, SourceMap};
use vun_term::{Align, Column, Record, Row, Style, Text};

use crate::Session;
use crate::testing::Covered;

/// Where a function begins in its file: line and column, counted from 1.
type Start = (u32, u32);

/// What a run reached of a file: whether each line holding code, and each function, was.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    pub lines: BTreeMap<u32, bool>,
    pub functions: BTreeMap<Start, bool>,
}

/// What a run reached of the files below a scope, by the path each is shown as.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    pub files: BTreeMap<PathBuf, Source>,
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
        for i in program.instantiations() {
            seen.insert(i.clone());
            self.record(&mut reach, &below, i, &|region| covered.ran(region.flag));
        }
        let sources = files.iter().filter(|path| path.file_name().is_some_and(|name| name != MANIFEST));
        let loaded: Vec<_> = sources.filter_map(|path| self.load(path)).collect();
        for file in loaded {
            for f in self.functions(file) {
                let i = Instantiation::plain(f);
                if !seen.contains(&i) {
                    self.record(&mut reach, &below, &i, &|_| false);
                }
            }
        }
        reach
    }

    /// Adds the lines and the function of the body of `i` written below `scope` to `reach`, a
    /// region reached when `ran` says so.
    fn record(&self, reach: &mut Reach, scope: &Path, i: &Instantiation, ran: &dyn Fn(&Region) -> bool) {
        let Some(regions) = Region::of(&Cx::new(&self.db), i) else { return };
        let holds = |path: &str| std::path::absolute(path).is_ok_and(|path| path.starts_with(scope));
        let start = self.sources.locate(self.source_span(i.f.literal.expr.origin));
        if holds(start.path) {
            let LineCol { line, col, .. } = start.at;
            let called = regions.iter().any(|region| region.entry && ran(region));
            *reach.source(start.path).functions.entry((line, col)).or_insert(false) |= called;
        }
        for region in &regions {
            for &origin in &region.origins {
                let at = self.sources.locate(self.source_span(origin));
                if holds(at.path) {
                    *reach.source(at.path).lines.entry(at.at.line).or_insert(false) |= ran(region);
                }
            }
        }
    }
}

impl Source {
    /// How many of `hits` are reached.
    #[must_use]
    pub fn reached<K>(hits: &BTreeMap<K, bool>) -> usize {
        hits.values().filter(|&&hit| hit).count()
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
                lines: Source::reached(&source.lines),
                of_lines: source.lines.len(),
                functions: Source::reached(&source.functions),
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
                source.functions.iter().map(move |(&(line, column), &hit)| FunctionRow {
                    path: path.display().to_string(),
                    line,
                    column,
                    hit,
                })
            })
            .collect()
    }
}

/// What a run reached of a file, in lines and in functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRow {
    pub path: String,
    pub lines: usize,
    pub of_lines: usize,
    pub functions: usize,
    pub of_functions: usize,
}

impl FileRow {
    /// The share of its lines reached, in percent; 100 for a file holding none.
    #[must_use]
    pub fn percent(&self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "a share, shown")]
        let share = |n: usize, of: usize| if of == 0 { 100.0 } else { n as f64 * 100.0 / of as f64 };
        share(self.lines, self.of_lines)
    }
}

impl Record for FileRow {
    fn kind(&self) -> &'static str {
        "covered_file"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.path.clone()).append(Text::from(format!(
            " {:.1}% of {} lines",
            self.percent(),
            self.of_lines
        )))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("lines", self.lines)
            .with("of_lines", self.of_lines)
            .with("functions", self.functions)
            .with("of_functions", self.of_functions)
    }
}

impl Row for FileRow {
    const COLUMNS: &'static [Column] = &[
        Column::left("file"),
        Column { head: "lines", align: Align::Right },
        Column { head: "of", align: Align::Right },
        Column { head: "%", align: Align::Right },
        Column { head: "functions", align: Align::Right },
        Column { head: "of", align: Align::Right },
    ];

    fn cells(&self) -> Vec<Text> {
        vec![
            Text::of(Style::Strong, self.path.clone()),
            Text::from(self.lines.to_string()),
            Text::from(self.of_lines.to_string()),
            Text::from(format!("{:.1}", self.percent())),
            Text::from(self.functions.to_string()),
            Text::from(self.of_functions.to_string()),
        ]
    }
}

/// Whether the run reached a function, which begins at a line and column of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionRow {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub hit: bool,
}

impl Record for FunctionRow {
    fn kind(&self) -> &'static str {
        "covered_function"
    }

    fn text(&self) -> Text {
        Text::from(format!("{}:{}:{}", self.path, self.line, self.column))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("line", self.line)
            .with("column", self.column)
            .with("hit", self.hit)
    }
}

impl Row for FunctionRow {
    const COLUMNS: &'static [Column] = &[Column::left("function"), Column::left("reached")];

    fn cells(&self) -> Vec<Text> {
        let reached = if self.hit { Text::of(Style::Good, "yes") } else { Text::of(Style::Error, "no") };
        vec![Text::from(format!("{}:{}:{}", self.path, self.line, self.column)), reached]
    }
}
