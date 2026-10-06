//! The host's side: a renderer that is a component, called through the boundary.

use std::borrow::Cow;
use std::io::Write;

use vun_component::{Component, Fault};
use vun_diag::{Color, DiagSink, Diagnostic};
use vun_origin::Resolve;
use vun_render_short::ShortRenderer;
use vun_span::SourceMap;
use vun_trace::tracing::{debug, warn};

use crate::{Answer, Ask, Call, Info, Renderer, Renderers, Show};

/// A renderer served by a component.
#[derive(Clone)]
pub struct Remote {
    component: Component,
    info: Info,
}

impl Remote {
    /// The renderer a component serves, its facts asked once and kept for the process.
    ///
    /// # Errors
    /// Returns why the component does not answer.
    pub fn of(component: Component) -> Result<Self, Fault> {
        let reply = component.call::<Renderers>(&Call::Info, &|ask| Self::unasked(&ask))?;
        let info = reply.info().map_err(Fault::Malformed)?;
        debug!(target: "vun_component", renderer = info.name, library = ?component.path(), "opened a renderer component");
        Ok(Self { component, info })
    }
}

impl Remote {
    /// What a call that has no files to ask about is answered with.
    fn unasked(ask: &Ask) -> Answer {
        panic!("a renderer asked {ask:?} of a call with no files")
    }
}

impl Renderer for Remote {
    fn name(&self) -> &'static str {
        self.info.name
    }

    fn about(&self) -> &'static str {
        self.info.about
    }

    fn sink<'w>(&self, out: &'w mut dyn Write, color: Color) -> Box<dyn DiagSink + 'w> {
        Box::new(Sink { remote: self.clone(), out, color })
    }
}

struct Sink<'w> {
    remote: Remote,
    out: &'w mut dyn Write,
    color: Color,
}

impl DiagSink for Sink<'_> {
    fn emit(&mut self, d: &Diagnostic, origins: &dyn Resolve, sources: &dyn SourceMap) {
        let colored = self.color == Color::Always;
        let call = Call::Render(Box::new(Show { diagnostic: Cow::Borrowed(d), colored }));
        let reply = self.remote.component.call::<Renderers>(&call, &|ask| ask.answered_by(origins));
        let rendered = reply.and_then(|reply| reply.rendered().map_err(Fault::Malformed));
        match rendered {
            // Best-effort output: a closed stream must not abort compilation.
            Ok(bytes) => drop(self.out.write_all(&bytes)),
            Err(fault) => {
                warn!(renderer = self.remote.info.name, %fault, "a renderer component failed; the diagnostic is shown in one line");
                ShortRenderer::new(&mut *self.out, self.color).emit(d, origins, sources);
            }
        }
    }
}
