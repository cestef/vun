//! What a run reached, as the records `vun cover` reports: a row per file and per function, and
//! the blocks sharing a count.

use vun_data::Object;
use vun_term::{Align, Column, Record, Row, Status, Style, Text, Verb};

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

/// How many times the run called a function, which begins at a line and column of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionRow {
    pub path: String,
    pub line: u32,
    pub column: u32,
    /// The path of its declaration, where it has one.
    pub name: Option<String>,
    pub calls: u64,
}

impl FunctionRow {
    /// What the function is called: its declaration's path, else where it begins.
    #[must_use]
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("{}:{}:{}", self.path, self.line, self.column))
    }
}

impl Record for FunctionRow {
    fn kind(&self) -> &'static str {
        "covered_function"
    }

    fn text(&self) -> Text {
        Text::from(self.label())
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("line", self.line)
            .with("column", self.column)
            .with("name", self.name.clone())
            .with("calls", self.calls)
    }
}

impl Row for FunctionRow {
    const COLUMNS: &'static [Column] =
        &[Column::left("function"), Column { head: "calls", align: Align::Right }];

    fn cells(&self) -> Vec<Text> {
        let calls = if self.calls > 0 { Style::Good } else { Style::Error };
        vec![Text::from(self.label()), Text::of(calls, self.calls.to_string())]
    }
}

/// Blocks of the program that share their count with another, so that what they report is not
/// theirs alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shared {
    pub blocks: usize,
}

impl Record for Shared {
    fn kind(&self) -> &'static str {
        "covered_shared"
    }

    fn text(&self) -> Text {
        Text::from(format!("{} blocks share a count with another, and report their sum", self.blocks))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("blocks", self.blocks)
    }
}

impl Status for Shared {
    fn verb(&self) -> Verb {
        Verb::warned("Shared")
    }
}
