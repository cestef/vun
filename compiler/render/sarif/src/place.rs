//! Where things are, as SARIF writes it: an artifact by URI, a region by lines and columns.

use std::path::Path;

use vun_data::{Object, ToData};
use vun_diag::Msg;
use vun_origin::{OriginId, Resolve};
use vun_span::{SourceMap, Span};
use vun_uri::{Reference, Uri};

/// What a relative artifact URI is relative to: the root of the sources the run read.
const SOURCE_ROOT: &str = "%SRCROOT%";

/// What the renderer needs to resolve locations.
pub(crate) struct Places<'a> {
    pub(crate) origins: &'a dyn Resolve,
    pub(crate) sources: &'a dyn SourceMap,
}

/// A path as shown to users, which an artifact names.
struct Artifact<'a>(&'a str);

impl Artifact<'_> {
    /// The `artifactLocation`: the file URI of an absolute path, else the path as a reference
    /// relative to [`SOURCE_ROOT`].
    fn location(&self) -> Object<'static> {
        if let Ok(uri) = Uri::from_file_path(Path::new(self.0)) {
            return Object::new().with("uri", uri.to_data());
        }
        let relative = Reference::builder().path(self.0).build();
        let uri = relative.map_or_else(|_| self.0.to_data(), |r| r.to_data());
        Object::new().with("uri", uri).with("uriBaseId", SOURCE_ROOT)
    }
}

impl Places<'_> {
    /// A `location`: where `span` is, with `message` when it says something.
    pub(crate) fn location(&self, span: Span, message: Option<&Msg>) -> Object<'static> {
        let physical =
            Object::new().with("artifactLocation", self.artifact(span)).with("region", self.region(span));
        let object = Object::new().with("physicalLocation", physical);
        match message {
            Some(msg) => object.with("message", text(msg)),
            None => object,
        }
    }

    /// The `location` of `origin`, as the files read now hold it.
    pub(crate) fn origin(&self, origin: OriginId, message: Option<&Msg>) -> Object<'static> {
        self.location(self.origins.source_span(origin), message)
    }

    pub(crate) fn artifact(&self, span: Span) -> Object<'static> {
        Artifact(&self.sources.file(span.file).path).location()
    }

    /// The `region` of `span`. A column counts characters, as editors do, and the end is the
    /// column after the last one; the bytes are kept apart.
    pub(crate) fn region(&self, span: Span) -> Object<'static> {
        let file = self.sources.file(span.file);
        let (start, end) = (file.line_col(span.range.lo), file.line_col(span.range.hi));
        Object::new()
            .with("startLine", start.line)
            .with("startColumn", start.chars)
            .with("endLine", end.line)
            .with("endColumn", end.chars)
            .with("byteOffset", span.range.lo)
            .with("byteLength", span.range.hi - span.range.lo)
    }
}

/// A `message` of plain text.
pub(crate) fn text(msg: &Msg) -> Object<'static> {
    Object::new().with("text", msg.to_string())
}
