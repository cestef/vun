//! The subscriber writing events to standard error: one line each, prefixed by the spans they
//! occur in (compact), or indented under them (tree).

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

use crate::{Config, Filter, Format};

/// The field `tracing` gives an event's message.
const MESSAGE: &str = "message";
/// The indentation of one span level in the tree format.
const INDENT: &str = "  ";

struct Span {
    name: &'static str,
    fields: String,
    /// Handles to the span; it closes when the last is dropped.
    handles: usize,
}

pub(crate) struct Writer {
    filter: Filter,
    format: Format,
    next: AtomicU64,
    spans: Mutex<HashMap<u64, Span>>,
}

thread_local! {
    /// The spans entered on this thread, innermost last.
    static ENTERED: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Collects fields as `name=value`, the message apart.
#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == MESSAGE {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }
}

impl Writer {
    pub(crate) fn new(config: &Config) -> Self {
        let filter = config.filter.as_deref().map(Filter::parse).unwrap_or_default();
        Self { filter, format: config.format, next: AtomicU64::new(1), spans: Mutex::default() }
    }

    fn line(text: &str) {
        let _ = writeln!(std::io::stderr().lock(), "{text}");
    }

    /// The spans this thread is in, outermost first, as `name{fields}`.
    fn context(&self) -> Vec<String> {
        let spans = self.spans.lock().expect("trace spans");
        ENTERED.with_borrow(|entered| {
            entered
                .iter()
                .filter_map(|id| spans.get(id))
                .map(|s| format!("{}{{{}}}", s.name, s.fields.trim_start()))
                .collect()
        })
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
            s.fields.push_str(&fields.rest);
        }
    }

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let meta = event.metadata();
        let what = format!("{} {}: {}{}", meta.level(), meta.target(), fields.message, fields.rest);
        let context = self.context();
        match self.format {
            Format::Compact if context.is_empty() => Self::line(&what),
            Format::Compact => Self::line(&format!("{} {what}", context.join(":"))),
            Format::Tree => {
                for (depth, span) in context.iter().enumerate() {
                    Self::line(&format!("{}{span}", INDENT.repeat(depth)));
                }
                Self::line(&format!("{}{what}", INDENT.repeat(context.len())));
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
