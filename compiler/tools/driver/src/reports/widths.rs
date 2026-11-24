//! The arithmetic intermediates wider than the target's register, as `vun check --widen` shows
//! them (spec 04, section 1).

use vun_data::Object;
use vun_mir_passes::widths::{Wide, WideChecks};
use vun_origin::Resolve;
use vun_span::{FileId, SourceMap};
use vun_term::{Align, Column, Record, Row, Style, Text};

use crate::Session;

/// A wide intermediate, and where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WideRow {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub bits: u64,
    pub register: u32,
}

impl Record for WideRow {
    fn kind(&self) -> &'static str {
        "wide"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, format!("{}:{}:{}", self.path, self.line, self.column))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("line", self.line)
            .with("column", self.column)
            .with("bits", self.bits)
            .with("register", self.register)
    }
}

impl Row for WideRow {
    const COLUMNS: &'static [Column] = &[
        Column::left("at"),
        Column { head: "bits", align: Align::Right },
        Column { head: "register", align: Align::Right },
    ];

    fn cells(&self) -> Vec<Text> {
        vec![self.text(), Text::from(self.bits.to_string()), Text::from(self.register.to_string())]
    }
}

impl Session {
    /// The wide intermediates of the bodies the program rooted at `file` compiles.
    pub(super) fn widths(&mut self, file: FileId) -> Vec<WideRow> {
        let mut rows = Vec::new();
        for body in self.reached_bodies(file) {
            let Some(wide) = self.db.get::<WideChecks>(&body) else { continue };
            for Wide { origin, bits, register } in wide {
                let at = self.sources.locate(self.source_span(origin));
                rows.push(WideRow {
                    path: at.path.to_owned(),
                    line: at.at.line,
                    column: at.at.col,
                    bits,
                    register,
                });
            }
        }
        rows
    }
}
