//! A diagnostic as annotate-snippets groups: the title group with every label, one group per
//! child that has labels or fixes, and one help group per fix.

use annotate_snippets::{AnnotationKind, Group, Level, Patch, Snippet};
use overt_diag::{Child, ChildKind, Diagnostic, Fix, Label, Severity};
use overt_origin::OriginTable;
use overt_span::{FileId, SourceMap, Span};

/// What the renderer needs to place labels and patches.
pub(crate) struct Cx<'a> {
    pub(crate) origins: &'a OriginTable,
    pub(crate) sources: &'a dyn SourceMap,
}

impl<'a> Cx<'a> {
    /// The groups showing `d`.
    pub(crate) fn groups(&self, d: &Diagnostic) -> Vec<Group<'a>> {
        let cx = self;
        let labels: Vec<(&Label, AnnotationKind)> = d
            .primary
            .iter()
            .map(|l| (l, AnnotationKind::Primary))
            .chain(d.labels.iter().map(|l| (l, AnnotationKind::Context)))
            .collect();
        let mut title =
            Group::with_title(level(d.severity).primary_title(d.message.to_string()).id(d.id.to_string()))
                .elements(cx.snippets(&labels));
        let mut groups = Vec::new();
        for child in &d.children {
            if child.labels.is_empty() && child.fixes.is_empty() {
                title = title.element(child_level(child.kind).message(child.msg.to_string()));
            } else {
                groups.push(cx.child(child));
            }
        }
        for frame in &d.trace {
            let at = cx.sources.locate(cx.origins.source_span(frame.origin));
            let msg = format!("{} at {at}", frame.msg);
            title = title.element(Level::NOTE.message(msg));
        }
        groups.insert(0, title);
        groups.extend(d.fixes.iter().map(|f| cx.fix(f)));
        groups
    }
}

impl<'a> Cx<'a> {
    /// One snippet per file, in order of first appearance, holding that file's labels.
    fn snippets(
        &self,
        labels: &[(&Label, AnnotationKind)],
    ) -> Vec<Snippet<'a, annotate_snippets::Annotation<'a>>> {
        let mut files: Vec<FileId> = Vec::new();
        let spans: Vec<Span> = labels.iter().map(|(l, _)| self.origins.source_span(l.origin)).collect();
        for span in &spans {
            if !files.contains(&span.file) {
                files.push(span.file);
            }
        }
        files
            .into_iter()
            .map(|id| {
                let file = self.sources.file(id);
                let annotations =
                    labels.iter().zip(&spans).filter(|(_, s)| s.file == id).map(|((l, kind), s)| {
                        let annotation = kind.span(s.range.lo as usize..s.range.hi as usize);
                        match &l.msg {
                            Some(msg) => annotation.label(msg.to_string()),
                            None => annotation,
                        }
                    });
                Snippet::source(file.text.as_str()).path(file.path.as_str()).annotations(annotations)
            })
            .collect()
    }

    fn child(&self, child: &Child) -> Group<'a> {
        let labels: Vec<_> = child.labels.iter().map(|l| (l, AnnotationKind::Context)).collect();
        let group = Group::with_title(child_level(child.kind).secondary_title(child.msg.to_string()))
            .elements(self.snippets(&labels));
        child.fixes.iter().fold(group, |group, fix| group.elements(self.patches(fix)))
    }

    fn fix(&self, fix: &Fix) -> Group<'a> {
        let title = format!("{} [{}]", fix.msg, fix.safety);
        Group::with_title(Level::HELP.secondary_title(title)).elements(self.patches(fix))
    }

    /// One snippet per file the fix edits.
    fn patches(&self, fix: &Fix) -> Vec<Snippet<'a, Patch<'a>>> {
        let mut files: Vec<FileId> = Vec::new();
        for edit in &fix.edits {
            if !files.contains(&edit.span.file) {
                files.push(edit.span.file);
            }
        }
        files
            .into_iter()
            .map(|id| {
                let file = self.sources.file(id);
                let patches =
                    fix.edits.iter().filter(|e| e.span.file == id).map(|e| {
                        Patch::new(e.span.range.lo as usize..e.span.range.hi as usize, e.text.clone())
                    });
                Snippet::source(file.text.as_str()).path(file.path.as_str()).patches(patches)
            })
            .collect()
    }
}

fn level(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
        Severity::Info => Level::INFO,
        Severity::Hint => Level::HELP,
    }
}

fn child_level(kind: ChildKind) -> Level<'static> {
    match kind {
        ChildKind::Note => Level::NOTE,
        ChildKind::Help => Level::HELP,
        ChildKind::Cause => Level::NOTE.with_name(Some(ChildKind::Cause.name())),
    }
}
