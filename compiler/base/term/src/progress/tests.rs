use std::io::{self, Write};

use std::sync::{Arc, Mutex};

use super::{Progress, Unit};
use crate::{Color, Noun, Text, Verb};

const FILES: Noun = Noun { one: "file", many: "files" };

/// A sink the test reads back.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Sink {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

fn drawn(sink: &Sink) -> Progress {
    Progress::drawn(Box::new(sink.clone()), Color::Never, 80)
}

#[test]
fn a_bar_shows_its_count_and_subject_and_goes_away_when_dropped() {
    let sink = Sink::default();
    let progress = drawn(&sink);
    let bar = progress.bar(Verb::good("Checking"), Text::from("net/tcp.vun"), 4, Unit::Items(FILES));
    bar.advance(2);
    progress.redraw();
    let shown = sink.text();
    assert!(shown.contains("Checking net/tcp.vun"), "{shown}");
    assert!(shown.contains("[=========>"), "{shown}");
    assert!(shown.contains("2/4 files"), "{shown}");
    assert!(shown.contains("elapsed"), "{shown}");
    drop(bar);
    assert!(sink.text().ends_with("\r\x1b[2K"), "{:?}", sink.text());
}

#[test]
fn a_stream_writing_finds_the_bars_off_the_terminal() {
    let sink = Sink::default();
    let progress = drawn(&sink);
    let _bar = progress.bar(Verb::good("Checking"), Text::new(), 0, Unit::Items(FILES));
    progress.redraw();
    let before = sink.text().len();
    let held = progress.hold();
    assert!(sink.text()[before..].starts_with("\r\x1b[2K"));
    drop(held);
}

#[test]
fn progress_that_is_off_draws_nothing() {
    let progress = Progress::off();
    let bar = progress.bar(Verb::good("Checking"), Text::new(), 3, Unit::Bytes);
    bar.advance(3);
    progress.redraw();
    assert!(progress.hold().is_none());
    assert_eq!(bar.done(), 3);
}

#[test]
fn bytes_are_counted_in_units() {
    let sink = Sink::default();
    let progress = drawn(&sink);
    let bar = progress.bar(Verb::good("Downloading"), Text::new(), 4096, Unit::Bytes);
    bar.advance(2048);
    progress.redraw();
    assert!(sink.text().contains("2.0 KiB/4.0 KiB"), "{}", sink.text());
}

#[test]
fn bars_are_not_drawn_in_the_middle_of_a_line_and_come_back_after_it() {
    use crate::Out;
    let (sink, text) = (Sink::default(), Sink::default());
    let progress = drawn(&sink);
    let _bar = progress.bar(Verb::good("Checking"), Text::new(), 0, Unit::Items(FILES));
    let mut writer = text.clone();
    let mut out = Out::plain(&mut writer);
    out.progress = progress.clone();
    out.write_all(b"half a li").unwrap();
    let drawn_before = sink.text().len();
    progress.redraw();
    assert_eq!(sink.text().len(), drawn_before, "drawn inside a line");
    out.write_all(b"ne\n").unwrap();
    progress.redraw();
    assert!(sink.text().len() > drawn_before);
    assert_eq!(text.text(), "half a line\n");
}

#[test]
fn a_watch_says_nothing_until_its_delay_and_while_nothing_runs() {
    use std::time::Duration;
    let sink = Sink::default();
    let progress = drawn(&sink);
    let _slow =
        progress.watch(Verb::good("Working"), Duration::from_secs(3600), Box::new(|| Text::from("code")));
    let _idle = progress.watch(Verb::good("Working"), Duration::ZERO, Box::new(Text::new));
    progress.redraw();
    assert_eq!(sink.text(), "");
    let _now = progress.watch(Verb::good("Working"), Duration::ZERO, Box::new(|| Text::from("code, link")));
    progress.redraw();
    assert!(sink.text().contains("Working code, link"), "{}", sink.text());
}
