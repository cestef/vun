//! `vun debug smith --jobs n`: the seeds split into chunks, each tried by a process of its own,
//! `n` at a time. A process keeps what it compiles, and dies with a program that kills it, so a
//! chunk is the unit that is both bounded and survivable.

use std::ffi::{OsStr, OsString};
use std::io;
use std::process::Command;
use std::process::ExitStatus;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use vun_command::Outcome;
use vun_data::Object;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

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
    let (next, failed) = (AtomicU64::new(0), AtomicU64::new(0));
    let lost = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(chunks) {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    if n >= chunks {
                        return;
                    }
                    let from = first + n * chunk;
                    let size = chunk.min(count - n * chunk);
                    let ran = Command::new(&exe)
                        .args(&common)
                        .args([
                            OsStr::new("--seed"),
                            from.to_string().as_ref(),
                            OsStr::new("--count"),
                            size.to_string().as_ref(),
                        ])
                        .status();
                    match ran {
                        Ok(status) if status.success() => {}
                        Ok(status) if status.code() != Some(1) => {
                            failed.fetch_add(1, Ordering::Relaxed);
                            lost.lock().expect("no worker panics holding it").push(Lost {
                                from,
                                size,
                                status,
                            });
                        }
                        _ => {
                            failed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    for lost in lost.into_inner().expect("no worker panics holding it") {
        shell.result(&lost)?;
    }
    let failed = failed.into_inner();
    shell.status(&Chunks { total: chunks, failed });
    Ok(if failed == 0 { Outcome::Success } else { Outcome::Failure })
}
