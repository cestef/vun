//! A diagnostic as a JSON value.

use vun_diag::{Child, Diagnostic, Edit, Fix, Label, Msg, MsgPart, TraceFrame};
use vun_json::{Number, Object, Value};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceMap, Span};

use crate::SCHEMA;

/// What the renderer needs to resolve locations.
pub(crate) struct Cx<'a> {
    pub(crate) origins: &'a OriginTable,
    pub(crate) sources: &'a dyn SourceMap,
}

impl Cx<'_> {
    pub(crate) fn diagnostic(&self, d: &Diagnostic) -> Value<'static> {
        let object = Object::new()
            .with("type", "diagnostic")
            .with("schema", SCHEMA)
            .with("id", d.id.to_string())
            .with("path", d.id.0.to_string())
            .with("severity", d.severity.to_string())
            .with("message", d.message.json())
            .with("primary", d.primary.as_ref().map(|l| self.label(l)))
            .with("labels", d.labels.iter().map(|l| self.label(l)).collect::<Vec<_>>())
            .with("children", d.children.iter().map(|c| self.child(c)).collect::<Vec<_>>())
            .with("fixes", d.fixes.iter().map(|f| self.fix(f)).collect::<Vec<_>>())
            .with("trace", d.trace.iter().map(|t| self.frame(t)).collect::<Vec<_>>());
        Value::Object(object)
    }

    fn label(&self, label: &Label) -> Value<'static> {
        let at = self.origin(label.origin);
        let msg = label.msg.as_ref().map(MsgJson::json);
        Value::Object(Object::new().with("at", at).with("message", msg))
    }

    fn child(&self, child: &Child) -> Value<'static> {
        let object = Object::new()
            .with("kind", child.kind.name())
            .with("message", child.msg.json())
            .with("labels", child.labels.iter().map(|l| self.label(l)).collect::<Vec<_>>())
            .with("fixes", child.fixes.iter().map(|f| self.fix(f)).collect::<Vec<_>>());
        Value::Object(object)
    }

    fn fix(&self, fix: &Fix) -> Value<'static> {
        let object = Object::new()
            .with("message", fix.msg.json())
            .with("safety", fix.safety.to_string())
            .with("preferred", fix.preferred)
            .with("edits", fix.edits.iter().map(|e| self.edit(e)).collect::<Vec<_>>());
        Value::Object(object)
    }

    fn edit(&self, edit: &Edit) -> Value<'static> {
        let holes: Vec<Value<'static>> = edit
            .holes
            .iter()
            .map(|h| {
                let range = Object::new().with("start", h.range.0).with("end", h.range.1);
                Value::Object(range.with("choices", h.choices.clone()))
            })
            .collect();
        let object = Object::new().with("at", self.span(edit.span)).with("text", edit.text.clone());
        Value::Object(object.with("holes", holes))
    }

    fn frame(&self, frame: &TraceFrame) -> Value<'static> {
        Value::Object(Object::new().with("message", frame.msg.json()).with("at", self.origin(frame.origin)))
    }

    fn origin(&self, origin: OriginId) -> Value<'static> {
        self.span(self.origins.source_span(origin))
    }

    /// Where `span` is: its file, its first and last positions, and its bytes.
    fn span(&self, span: Span) -> Value<'static> {
        let file = self.sources.file(span.file);
        let (start, end) = (file.line_col(span.range.lo), file.line_col(span.range.hi));
        let position =
            |at: vun_span::LineCol| Value::Object(Object::new().with("line", at.line).with("column", at.col));
        let bytes = Object::new().with("start", span.range.lo).with("end", span.range.hi);
        let object = Object::new()
            .with("file", file.path.clone())
            .with("start", position(start))
            .with("end", position(end))
            .with("bytes", bytes);
        Value::Object(object)
    }
}

/// Messages as JSON.
trait MsgJson {
    /// Its text, and its parts, each naming what it is.
    fn json(&self) -> Value<'static>;
}

impl MsgJson for Msg {
    fn json(&self) -> Value<'static> {
        let parts: Vec<Value<'static>> = self
            .0
            .iter()
            .map(|part| {
                let (kind, value): (&str, Value<'static>) = match part {
                    MsgPart::Text(text) => ("text", text.to_string().into()),
                    MsgPart::Code(code) => ("code", code.clone().into()),
                    MsgPart::Type(ty) => ("type", ty.clone().into()),
                    // An integer of any size, kept exact.
                    MsgPart::Num(n) => (
                        "number",
                        Number::parse(&n.to_string()).map_or(Value::Null, |n| Value::Number(n).into_owned()),
                    ),
                };
                Value::Object(Object::new().with("kind", kind).with("value", value))
            })
            .collect();
        Value::Object(Object::new().with("text", self.to_string()).with("parts", parts))
    }
}
