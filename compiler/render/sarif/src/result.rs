//! A diagnostic as a SARIF `result`, with the model's labels, trace, children and fixes as the
//! places SARIF has for them.

use vun_data::{Object, Value};
use vun_diag::{Child, Diagnostic, Edit, Fix, Severity};
use vun_span::FileId;

use crate::place::{Places, text};

/// A result's `level`.
#[derive(Clone, Copy)]
enum Level {
    Note,
    Warning,
    Error,
}

impl Level {
    const fn name(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl From<Severity> for Level {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Hint | Severity::Info => Self::Note,
            Severity::Warning => Self::Warning,
            Severity::Error => Self::Error,
        }
    }
}

impl Places<'_> {
    /// The `result` of `d`, without the index of its rule, which the document gives.
    pub(crate) fn result(&self, d: &Diagnostic) -> Object<'static> {
        let related = self.related(d);
        let fixes = d.fixes.iter().chain(d.children.iter().flat_map(|c| &c.fixes));
        let mut object = Object::new()
            .with("ruleId", d.id.to_string())
            .with("level", Level::from(d.severity).name())
            .with("message", text(&d.message))
            .with("locations", d.primary.iter().map(|l| self.origin(l.origin, None)).collect::<Vec<_>>());
        if !related.is_empty() {
            object.push("relatedLocations", related);
        }
        let fixes: Vec<_> = fixes.map(|f| self.fix(f)).collect();
        if !fixes.is_empty() {
            object.push("fixes", fixes);
        }
        if !d.children.is_empty() {
            let children: Vec<_> = d.children.iter().map(Self::child).collect();
            object.push("properties", Object::new().with("children", children));
        }
        object
    }

    /// The other labels, the labels of the children and the trace, innermost first, each with its
    /// own `id` for what refers to it.
    fn related(&self, d: &Diagnostic) -> Vec<Object<'static>> {
        let labels = d.labels.iter().chain(d.children.iter().flat_map(|c| &c.labels));
        let labelled = labels.map(|l| self.origin(l.origin, l.msg.as_ref()));
        let traced = d.trace.iter().map(|frame| self.origin(frame.origin, Some(&frame.msg)));
        labelled.chain(traced).enumerate().map(|(id, location)| location.with("id", id)).collect()
    }

    /// A child as SARIF has no place for it: its kind and message, kept as a property.
    fn child(child: &Child) -> Object<'static> {
        Object::new().with("kind", child.kind.name()).with("message", text(&child.msg))
    }

    /// A `fix`: its edits as the replacements of the artifacts they change.
    fn fix(&self, fix: &Fix) -> Object<'static> {
        let mut changes: Vec<(FileId, Vec<&Edit>)> = Vec::new();
        for edit in &fix.edits {
            match changes.iter_mut().find(|(file, _)| *file == edit.span.file) {
                Some((_, edits)) => edits.push(edit),
                None => changes.push((edit.span.file, vec![edit])),
            }
        }
        let changes: Vec<Value<'static>> = changes
            .into_iter()
            .map(|(_, edits)| {
                let replacements: Vec<_> = edits.iter().map(|e| self.replacement(e)).collect();
                let artifact = self.artifact(edits[0].span);
                Value::Object(
                    Object::new().with("artifactLocation", artifact).with("replacements", replacements),
                )
            })
            .collect();
        let properties =
            Object::new().with("safety", fix.safety.to_string()).with("preferred", fix.preferred);
        Object::new()
            .with("description", text(&fix.msg))
            .with("artifactChanges", changes)
            .with("properties", properties)
    }

    fn replacement(&self, edit: &Edit) -> Object<'static> {
        let inserted = Object::new().with("text", edit.text.clone());
        Object::new().with("deletedRegion", self.region(edit.span)).with("insertedContent", inserted)
    }
}
