//! Drawing the bars: one line each, the cursor left at the end of the last.

use std::io::Write;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::State;
use super::bar::{Task, Unit};
use crate::{Bytes, Elapsed, Out, Style, Text};

/// The columns between the brackets of a bar.
const BAR: usize = 20;
/// How many columns of the subject are shown at least.
const SUBJECT: usize = 12;
/// How many columns the subject takes at most, so the bar stays where it is.
const SUBJECT_COLUMN: usize = 32;
/// The time a marker of a task of unknown size takes to cross one column.
const SWEEP: Duration = Duration::from_millis(80);

const ERASE_LINE: &[u8] = b"\r\x1b[2K";
const LINE_UP: &[u8] = b"\x1b[1A";

impl State {
    /// Takes the bars off the terminal.
    pub(super) fn erase(&mut self) {
        if self.shown == 0 {
            return;
        }
        let mut frame = ERASE_LINE.to_vec();
        for _ in 1..self.shown {
            frame.extend_from_slice(LINE_UP);
            frame.extend_from_slice(ERASE_LINE);
        }
        self.shown = 0;
        let _ = self.sink.write_all(&frame).and_then(|()| self.sink.flush());
    }

    /// Draws the bars where the erased ones were.
    pub(super) fn draw(&mut self) {
        self.erase();
        if self.tasks.is_empty() || !self.at_line_start {
            return;
        }
        let mut frame = Vec::new();
        let lines: Vec<Text> = self.tasks.iter().filter_map(|task| task.line(self.width)).collect();
        for (at, line) in lines.iter().enumerate() {
            if at > 0 {
                frame.push(b'\n');
            }
            let _ = Out::colored(&mut frame, self.color).print(line);
        }
        self.shown = lines.len();
        let _ = self.sink.write_all(&frame).and_then(|()| self.sink.flush());
    }
}

impl Task {
    /// The line of the task, if it is to be drawn yet.
    fn line(&self, width: usize) -> Option<Text> {
        if self.started.elapsed() < self.delay {
            return None;
        }
        if let Some(live) = &self.live {
            let stages = live();
            return (stages.width() > 0).then(|| self.watching(&stages, width));
        }
        Some(self.progress_line(width))
    }

    /// `Working code, compile 3.20s`.
    fn watching(&self, stages: &Text, width: usize) -> Text {
        let elapsed = Elapsed(self.started.elapsed()).to_string();
        let room = width.saturating_sub(crate::Verb::WIDTH + 2 + elapsed.len()).max(SUBJECT);
        self.verb.line(Self::cut(stages, room).plain(" ").styled(Style::Dim, elapsed))
    }

    /// `Checking net/tcp.vun [=====>    ] 2/4 files, 1.20s left, 3.20s elapsed`: the subject first
    /// and the details after it, as a finished step reads.
    fn progress_line(&self, width: usize) -> Text {
        let (done, total) = (self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed));
        let elapsed = self.started.elapsed();
        let eta = (total > done && done > 0).then(|| {
            let nanos = elapsed.as_nanos() * u128::from(total - done) / u128::from(done);
            Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
        });
        let details = Text::join(
            [
                Some(self.counts(done, total)),
                eta.map(|eta| Self::timed(eta, "left")),
                Some(Self::timed(elapsed, "elapsed")),
            ]
            .into_iter()
            .flatten(),
            ", ",
        );
        let subject = self.subject.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let text = if subject.width() == 0 {
            Text::new()
        } else {
            let fixed = crate::Verb::WIDTH + 1 + BAR + 2 + details.width() + 2;
            let room = width.saturating_sub(fixed).clamp(SUBJECT, SUBJECT_COLUMN);
            let cut = Self::cut(&subject, room);
            let pad = room.saturating_sub(cut.width());
            cut.pad(pad + 1)
        };
        self.verb.line(text.append(Self::bar(done, total, elapsed)).plain(" ").append(details))
    }

    /// `2/4 files` or `2.0 KiB/4.0 KiB`, the unit dim.
    fn counts(&self, done: u64, total: u64) -> Text {
        match self.unit {
            Unit::Items(noun) if total > 0 => {
                Text::from(format!("{done}/{total}")).styled(Style::Dim, format!(" {}", noun.many))
            }
            Unit::Items(noun) => Text::from(done.to_string()).styled(Style::Dim, format!(" {}", noun.many)),
            Unit::Bytes if total > 0 => Text::from(format!("{}/{}", Bytes(done), Bytes(total))),
            Unit::Bytes => Text::from(Bytes(done).to_string()),
        }
    }

    /// `1.20s left`, the label dim.
    fn timed(time: Duration, label: &'static str) -> Text {
        Text::from(Elapsed(time).to_string()).styled(Style::Dim, format!(" {label}"))
    }

    /// `[=====>    ]` for a known total, a marker sweeping to and fro for an unknown one.
    fn bar(done: u64, total: u64, elapsed: Duration) -> Text {
        let known = (done.min(total) * BAR as u64).checked_div(total);
        let (lead, marked) = if let Some(filled) = known {
            let filled = usize::try_from(filled).unwrap_or(BAR);
            (0, if filled > 0 { "=".repeat(filled - 1) + ">" } else { String::new() })
        } else {
            let span = 2 * (BAR - 1);
            let at = usize::try_from(elapsed.as_millis() / SWEEP.as_millis()).unwrap_or(0) % span;
            (if at < BAR { at } else { span - at }, "=".to_owned())
        };
        let blank = BAR.saturating_sub(lead + marked.chars().count());
        Text::new()
            .plain("[")
            .plain(" ".repeat(lead))
            .styled(Style::Good, marked)
            .plain(" ".repeat(blank))
            .plain("]")
    }

    /// `subject` cut to `room` columns, its start shown when it does not fit.
    fn cut(subject: &Text, room: usize) -> Text {
        let width = subject.width();
        if width <= room {
            return subject.clone();
        }
        let whole = subject.to_string();
        let kept: String = whole.chars().skip(width - room.saturating_sub(1)).collect();
        Text::new().styled(Style::Dim, "…").plain(kept)
    }
}
