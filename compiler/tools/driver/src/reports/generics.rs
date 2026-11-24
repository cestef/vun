//! The generic functions a program reaches, how often each is instantiated and how much code
//! the instantiations make (spec 09, section 4): MIR statements stand for the code, which
//! lowering and the backends make of each by the same rules whatever the function.

use std::collections::HashMap;

use vun_data::Object;
use vun_mir::{Mir, Programs, Roots};
use vun_origin::{OriginTable, Resolve};
use vun_query::Cx;
use vun_span::{FileId, SourceMap};
use vun_term::{Align, Column, Record, Row, Style, Text};

use crate::Session;

/// A generic function: where it is written, and what its instantiations came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generic {
    pub path: String,
    pub line: u32,
    pub column: u32,
    /// The path of its declaration, where it has one.
    pub name: Option<String>,
    pub instantiations: usize,
    /// The MIR statements of all its instantiations.
    pub statements: usize,
}

impl Generic {
    /// What the function is called: its declaration's path, else where it begins.
    #[must_use]
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("{}:{}:{}", self.path, self.line, self.column))
    }
}

impl Record for Generic {
    fn kind(&self) -> &'static str {
        "generic"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.label())
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("line", self.line)
            .with("column", self.column)
            .with("name", self.name.clone())
            .with("instantiations", self.instantiations)
            .with("statements", self.statements)
    }
}

impl Row for Generic {
    const COLUMNS: &'static [Column] = &[
        Column::left("generic"),
        Column { head: "instantiations", align: Align::Right },
        Column { head: "statements", align: Align::Right },
    ];

    fn cells(&self) -> Vec<Text> {
        vec![
            Text::of(Style::Strong, self.label()),
            Text::from(self.instantiations.to_string()),
            Text::from(self.statements.to_string()),
        ]
    }
}

impl Session {
    /// The generic functions the program rooted at `file` reaches, a row each.
    pub(super) fn generics(&mut self, file: FileId) -> Vec<Generic> {
        let root = self.file_ref(file);
        let program = self.db.get::<Programs>(&(root, Roots::Functions));
        let cx = Cx::new(&self.db);
        let mut found: HashMap<_, Generic> = HashMap::new();
        for i in program.instantiations().filter(|i| !i.args.is_empty()) {
            let origin = i.f.literal.expr.origin;
            let statements = cx
                .get::<Mir>(i)
                .map_or(0, |body| body.blocks.iter().map(|(_, block)| block.stmts.len()).sum());
            let generic = found.entry(origin).or_insert_with(|| {
                let start = self.sources.locate(self.source_span(origin));
                let name = OriginTable::global().declaration_name(origin);
                Generic {
                    path: start.path.to_owned(),
                    line: start.at.line,
                    column: start.at.col,
                    name: name.map(|n| n.as_str().to_owned()),
                    instantiations: 0,
                    statements: 0,
                }
            });
            generic.instantiations += 1;
            generic.statements += statements;
        }
        found.into_values().collect()
    }
}
