//! `vun debug smith --jobs n`: the seeds split into chunks, each tried by a process of its own,
//! `n` at a time. A process keeps what it compiles, and dies with a program that kills it, so a
//! chunk is the unit that is both bounded and survivable.

use std::ffi::{OsStr, OsString};
use std::io;
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use vun_command::Outcome;
use vun_data::Object;
use vun_term::{Noun, Record, Shell, Status, Style, Text, Unit, Verb};

use super::smith::{CHUNK, COUNT, JOBS, SEED};

/// What the chunks came to.
struct Chunks {
    total: u64,
    failed: u64,
}

impl Record for Chunks {
    fn kind(&self) -> &'static str {
        "chunks"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, format!("{} chunks, {} found something", self.total, self.failed))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("chunks", self.total).with("failed", self.failed)
    }
}

impl Status for Chunks {
    fn verb(&self) -> Verb {
        if self.failed == 0 { Verb::good("Agreed") } else { Verb::failed("Found") }
    }
}

/// A chunk whose process ended other than by saying what it found: killed, or out of memory.
struct Lost {
    from: u64,
    size: u64,
    status: ExitStatus,
}

impl Record for Lost {
    fn kind(&self) -> &'static str {
        "lost"
    }

    fn text(&self) -> Text {
        let seeds = format!("seeds {}..{}", self.from, self.from + self.size);
        Text::of(Style::Strong, seeds)
            .plain(": the process ended, ")
            .styled(Style::Error, self.status.to_string())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("from", self.from).with("size", self.size).with("status", self.status.to_string())
    }
}

/// The arguments this process was started with, without the options that say which seeds and
/// how many processes: what each worker is asked besides its seeds.
fn kept(args: impl Iterator<Item = OsString>) -> Vec<OsString> {
    let dropped: Vec<String> = [SEED, COUNT, JOBS, CHUNK].iter().map(|p| format!("--{}", p.name)).collect();
    let mut kept = Vec::new();
    let mut args = args;
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        let name = text.split('=').next().unwrap_or_default();
        if dropped.iter().any(|d| d == name) {
            if !text.contains('=') {
                args.next();
            }
        } else {
            kept.push(arg);
        }
    }
    kept
}

/// The programs a chunk is made of, as the bar counts them.
const PROGRAMS: Noun = Noun { one: "program", many: "programs" };

/// A chunk whose process ended: what it wrote, and how.
struct Done {
    from: u64,
    size: u64,
    ran: io::Result<Output>,
}

pub(super) fn run(
    first: u64,
    count: u64,
    chunk: u64,
    jobs: u64,
    shell: &mut Shell<'_>,
) -> io::Result<Outcome> {
    let exe = std::env::current_exe()?;
    let common = kept(std::env::args_os().skip(1));
    let chunk = chunk.max(1);
    let chunks = count.div_ceil(chunk);
    let next = AtomicU64::new(0);
    let (workers, tried) =
        (jobs.min(chunks), shell.bar(Verb::good("Trying"), Text::new(), count, Unit::Items(PROGRAMS)));
    let (mut failed, mut lost) = (0, Vec::new());
    let (send, receive) = mpsc::channel();
    // A worker's words are shown here, one chunk at a time, where the bar makes room for them.
    std::thread::scope(|scope| -> io::Result<()> {
        for _ in 0..workers {
            let (send, next, exe, common) = (send.clone(), &next, &exe, &common);
            scope.spawn(move || {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    if n >= chunks {
                        return;
                    }
                    let from = first + n * chunk;
                    let size = chunk.min(count - n * chunk);
                    let ran = Command::new(exe)
                        .args(common)
                        .args([
                            OsStr::new("--seed"),
                            from.to_string().as_ref(),
                            OsStr::new("--count"),
                            size.to_string().as_ref(),
                        ])
                        .output();
                    if send.send(Done { from, size, ran }).is_err() {
                        return;
                    }
                }
            });
        }
        drop(send);
        for Done { from, size, ran } in receive {
            tried.advance(size);
            tried.subject(Text::from(format!("seeds {from}..{}", from + size)));
            match ran {
                Ok(output) => {
                    shell.out.writer().write_all(&output.stdout)?;
                    shell.err.writer().write_all(&output.stderr)?;
                    match output.status {
                        status if status.success() => {}
                        status if status.code() != Some(1) => {
                            failed += 1;
                            lost.push(Lost { from, size, status });
                        }
                        _ => failed += 1,
                    }
                }
                Err(_) => failed += 1,
            }
        }
        Ok(())
    })?;
    drop(tried);
    for lost in lost {
        shell.result(&lost)?;
    }
    shell.status(&Chunks { total: chunks, failed });
    Ok(if failed == 0 { Outcome::Success } else { Outcome::Failure })
}
