//! The subscriber writing events to standard error: one line each, prefixed by the spans they
//! occur in (compact), or indented under them (tree).

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};
use vun_term::{Color, Out, Style, Text};

use crate::{Config, Filter, Format};

/// The field `tracing` gives an event's message.
const MESSAGE: &str = "message";
/// The indentation of one span level in the tree format.
const INDENT: &str = "  ";

struct Span {
    name: &'static str,
    fields: Pairs,
    /// Handles to the span; it closes when the last is dropped.
    handles: usize,
}

pub(crate) struct Writer {
    filter: Filter,
    format: Format,
    color: Color,
    next: AtomicU64,
    spans: Mutex<HashMap<u64, Span>>,
}

thread_local! {
    /// The spans entered on this thread, innermost last.
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Collects fields, the message apart.
#[derive(Default)]
struct Fields {
    message: String,
    rest: Pairs,
}

/// Fields as names and values.
#[derive(Default)]
struct Pairs(Vec<(&'static str, String)>);

impl Pairs {
    /// ` name=value` each, the name as a key, the value as written.
    fn text(&self) -> Text {
        self.0.iter().fold(Text::new(), |text, (name, value)| {
            text.plain(" ").styled(Style::Key, *name).styled(Style::Dim, "=").plain(value.clone())
        })
    }
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == MESSAGE {
            let _ = write!(self.message, "{value:?}");
        } else {
            self.rest.0.push((field.name(), format!("{value:?}")));
        }
    }
}

impl Writer {
    pub(crate) fn new(config: &Config) -> Self {
        let filter = config.filter.as_deref().map(Filter::from).unwrap_or_default();
        Self {
            filter,
            format: config.format,
            color: config.color,
            next: AtomicU64::new(1),
            spans: Mutex::default(),
        }
    }

    fn line(&self, text: &Text) {
        let _ = Out::colored(&mut std::io::stderr().lock(), self.color).line(text);
    }

    /// The spans this thread is in, outermost first, as `name{fields}`.
    fn context(&self) -> Vec<Text> {
        let spans = self.spans.lock().expect("trace spans");
        ENTERED.with_borrow(|entered| {
            entered
                .iter()
                .filter_map(|id| spans.get(id))
                .map(|s| {
                    let name = Text::of(Style::Scope, s.name);
                    if s.fields.0.is_empty() {
                        name
                    } else {
                        name.styled(Style::Dim, "{").append(s.fields.text()).styled(Style::Dim, " }")
                    }
                })
                .collect()
        })
    }
}

/// How log lines style a level, as `tracing-subscriber` colors them.
trait LevelStyle {
    fn style(&self) -> Style;
}

impl LevelStyle for Level {
    fn style(&self) -> Style {
        match *self {
            Level::ERROR => Style::Error,
            Level::WARN => Style::Warning,
            Level::INFO => Style::Good,
            Level::DEBUG => Style::Accent,
            Level::TRACE => Style::Id,
        }
    }
}

impl Subscriber for Writer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata.target(), *metadata.level())
    }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let span = Span { name: attributes.metadata().name(), fields: fields.rest, handles: 1 };
        self.spans.lock().expect("trace spans").insert(id, span);
        Id::from_u64(id)
    }

    fn record(&self, span: &Id, values: &Record<'_>) {
        let mut fields = Fields::default();
        values.record(&mut fields);
        if let Some(s) = self.spans.lock().expect("trace spans").get_mut(&span.into_u64()) {
            s.fields.0.extend(fields.rest.0);
        }
    }

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let meta = event.metadata();
        let level = Text::of(meta.level().style(), format!("{:>5}", meta.level())).plain(" ");
        let what = Text::of(Style::Scope, meta.target()).styled(Style::Dim, ": ");
        let what = what.plain(fields.message).append(fields.rest.text());
        let context = self.context();
        match self.format {
            Format::Compact if context.is_empty() => self.line(&level.append(what)),
            Format::Compact => {
                let spans = Text::join(context, " > ").styled(Style::Dim, ": ");
                self.line(&level.append(spans).append(what));
            }
            Format::Tree => {
                let depth = context.len();
                for (at, span) in context.into_iter().enumerate() {
                    self.line(&Text::new().plain(INDENT.repeat(at)).append(span));
                }
                self.line(&Text::new().plain(INDENT.repeat(depth)).append(level).append(what));
            }
        }
    }

    fn enter(&self, span: &Id) {
        ENTERED.with_borrow_mut(|entered| entered.push(span.into_u64()));
    }

    fn exit(&self, span: &Id) {
        ENTERED.with_borrow_mut(|entered| {
            if let Some(at) = entered.iter().rposition(|&id| id == span.into_u64()) {
                entered.remove(at);
            }
        });
    }

    fn clone_span(&self, span: &Id) -> Id {
        if let Some(s) = self.spans.lock().expect("trace spans").get_mut(&span.into_u64()) {
            s.handles += 1;
        }
        span.clone()
    }

    fn try_close(&self, span: Id) -> bool {
        let mut spans = self.spans.lock().expect("trace spans");
        let Some(s) = spans.get_mut(&span.into_u64()) else { return false };
        s.handles -= 1;
        if s.handles > 0 {
            return false;
        }
        spans.remove(&span.into_u64());
        true
    }
}
