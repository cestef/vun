//! The library's side: a renderer answering the host's calls, the files it reads asked of the
//! host where they were read again.

use vun_codec::Malformed;
use vun_component::{Asked, Interface, Kind};
use vun_diag::Color;
use vun_origin::{OriginTable, Resolve};
use vun_span::{FileId, SourceFiles};

use crate::{Answer, Ask, Call, Info, Renderer, Reply, Show};

/// The interface of renderers.
pub struct Renderers;

impl Interface for Renderers {
    const KIND: Kind = Kind("renderer");
    type Service = dyn Renderer;
    type Call<'a> = Call<'a>;
    type Reply = Reply;
    type Ask = Ask;
    type Answer = Answer;

    fn serve(renderer: &Self::Service, call: Call<'static>, ask: Asked<'_, Self>) -> Reply {
        match call {
            Call::Info => Reply::Info(Info { name: renderer.name(), about: renderer.about() }),
            Call::Render(show) => {
                let Show { diagnostic, colored } = *show;
                let color = if colored { Color::Always } else { Color::Never };
                let mut out = Vec::new();
                renderer.sink(&mut out, color).emit(&diagnostic, &Asking(ask), SourceFiles::global());
                Reply::Rendered(out)
            }
            Call::Finish => {
                let mut out = Vec::new();
                renderer.finish(&mut out);
                Reply::Rendered(out)
            }
        }
    }
}

/// The origins the host sent, as this library holds them, and the host's answers about its files.
struct Asking<'a>(Asked<'a, Renderers>);

impl Resolve for Asking<'_> {
    fn origins(&self) -> &OriginTable {
        OriginTable::global()
    }

    fn current(&self, file: FileId) -> FileId {
        let question = format!("{:?}", Ask::Current(file));
        (self.0)(Ask::Current(file))
            .current()
            .unwrap_or_else(|e: Malformed| panic!("the host answered {question} with a wrong answer: {e}"))
    }
}
