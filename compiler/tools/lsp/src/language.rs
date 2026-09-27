//! What the server tells of a position: the type of the expression there, and where the name
//! there is declared.

use std::path::Path;

use vun_driver::editor::Target;
use vun_json::{Object, Value};

use crate::message::Failure;
use crate::position::Lines;
use crate::server::Server;
use crate::uri::Uri;

/// The fence the type of a hover is shown in, as Markdown.
const FENCE: &str = "```";
/// The language Markdown fences name.
const LANGUAGE: &str = "vun";

impl Server<'_> {
    /// The document and byte offset `params` name.
    fn at(&self, params: &Value<'_>) -> Result<(vun_span::FileId, u32), Failure> {
        let (_, document) = self.document(params)?;
        let position = params.get("position").ok_or(Failure::invalid("position"))?;
        let text = self.session.text(document.file);
        let offset =
            document.lines.offset(text, position, self.encoding).ok_or(Failure::invalid("position"))?;
        Ok((document.file, offset))
    }

    pub(crate) fn hover(&mut self, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let (file, offset) = self.at(params)?;
        let Some(hovered) = self.session.hovered(file, offset) else { return Ok(Value::Null) };
        let mut shown = Vec::new();
        if let Some(known) = hovered.known {
            shown.push(format!("{FENCE}{LANGUAGE}\n{}\n{FENCE}\n{}", known.ty, known.stage.name()));
        }
        if !hovered.doc.is_empty() {
            shown.push(hovered.doc.join("\n"));
        }
        if shown.is_empty() {
            return Ok(Value::Null);
        }
        let text = self.session.text(file);
        let range = Lines::of(text).range(text, hovered.span.range.lo, hovered.span.range.hi, self.encoding);
        let contents = Object::new().with("kind", "markdown").with("value", shown.join("\n\n"));
        Ok(Object::new().with("contents", contents).with("range", range).into())
    }

    pub(crate) fn definition(&mut self, params: &Value<'_>) -> Result<Value<'static>, Failure> {
        let (file, offset) = self.at(params)?;
        Ok(match self.session.declared(file, offset) {
            Some(Target::At(span)) => self.location(span),
            Some(Target::File(path)) => Self::start_of(&path),
            None => Value::Null,
        })
    }

    /// The location of the start of the file at `path`.
    fn start_of(path: &Path) -> Value<'static> {
        let start = Object::new().with("line", 0).with("character", 0);
        let range = Object::new().with("start", start.clone()).with("end", start);
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        Object::new().with("uri", Uri::of(&path).as_str().to_owned()).with("range", range).into()
    }
}
