//! Aligned columns of styled text: help sections, listings, reports.

use crate::{Style, Text};

/// Spaces between two columns.
const GAP: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// The head shown above the column; a table whose heads are all empty shows none.
    pub head: &'static str,
    pub align: Align,
}

impl Column {
    #[must_use]
    pub const fn left(head: &'static str) -> Self {
        Self { head, align: Align::Left }
    }

    #[must_use]
    pub const fn right(head: &'static str) -> Self {
        Self { head, align: Align::Right }
    }
}

#[derive(Clone, Debug)]
pub struct Table<'c> {
    columns: &'c [Column],
    rows: Vec<Vec<Text>>,
    indent: usize,
}

impl<'c> Table<'c> {
    #[must_use]
    pub fn new(columns: &'c [Column]) -> Self {
        Self { columns, rows: Vec::new(), indent: 0 }
    }

    /// Spaces before every row.
    #[must_use]
    pub fn indent(self, indent: usize) -> Self {
        Self { indent, ..self }
    }

    /// With `rows` added.
    #[must_use]
    pub fn rows<R: IntoIterator<Item = Text>>(mut self, rows: impl IntoIterator<Item = R>) -> Self {
        rows.into_iter().for_each(|row| self.row(row));
        self
    }

    /// Adds a row, one cell per column.
    pub fn row(&mut self, cells: impl IntoIterator<Item = Text>) {
        let cells: Vec<Text> = cells.into_iter().collect();
        debug_assert_eq!(cells.len(), self.columns.len(), "a cell per column");
        self.rows.push(cells);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The table, one line per row, each ending in a newline; nothing, heads included, when it
    /// has no rows.
    #[must_use]
    pub fn text(self) -> Text {
        if self.rows.is_empty() {
            return Text::new();
        }
        let head = self.columns.iter().any(|c| !c.head.is_empty()).then(|| {
            self.columns.iter().map(|c| Text::new().styled(Style::Heading, c.head)).collect::<Vec<_>>()
        });
        let rows: Vec<Vec<Text>> = head.into_iter().chain(self.rows).collect();
        let widths: Vec<usize> =
            (0..self.columns.len()).map(|i| rows.iter().map(|r| r[i].width()).max().unwrap_or(0)).collect();
        let last = self.columns.len().saturating_sub(1);
        rows.into_iter().fold(Text::new(), |text, row| {
            let line = row.into_iter().zip(self.columns).zip(&widths).enumerate().fold(
                Text::new().pad(self.indent),
                |line, (i, ((cell, column), &width))| {
                    let line = if i == 0 { line } else { line.pad(GAP) };
                    let room = width - cell.width();
                    match column.align {
                        Align::Right => line.pad(room).append(cell),
                        Align::Left if i == last => line.append(cell),
                        Align::Left => line.append(cell).pad(room),
                    }
                },
            );
            text.append(line).newline()
        })
    }
}
