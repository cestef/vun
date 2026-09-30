//! Reserving a host's fastest cores for a run: those the kernel gives the highest capacity
//! (`cpu_capacity`, which tells the kinds of core of a mixed machine apart) run the command, and
//! the host's services are confined to the others meanwhile. The confinement is a runtime
//! property of systemd, undone once the run ends, and by a reboot.

use std::fmt;
use std::path::Path;

use vun_data::Object;
use vun_diag::{DiagId, Diagnostic, Msg};
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Streams};

use super::Mirror;
use crate::task::Program;

/// Each core's capacity, one `<its directory>:<capacity>` line per core.
const CAPACITIES: [&str; 3] = ["grep", "--with-filename", "."];
const CAPACITY_FILES: &str = "/sys/devices/system/cpu/cpu[0-9]*/cpu_capacity";
/// Where a core's directory name puts its number.
const CORE: &str = "cpu";
/// The slice systemd runs the host's services in.
const SERVICES: &str = "system.slice";
const ALLOWED: &str = "AllowedCPUs";
const SHOW: [&str; 4] = ["systemctl", "show", "--value", "--property"];
const SET: [&str; 5] = ["sudo", "--non-interactive", "systemctl", "set-property", "--runtime"];
/// Moves the remote shell, and so every command it runs, onto the cores reserved.
const PIN: [&str; 3] = ["taskset", "--cpu-list", "--pid"];
const PINNED: [&str; 2] = ["$$", ">/dev/null"];

const UNRESERVED: DiagId = DiagId::new("xtask.remote.unreserved");

/// The fastest cores of a host, reserved until [`Reservation::release`].
pub(crate) struct Reservation {
    host: String,
    fastest: Cores,
    /// The services' cores before, restored after.
    before: String,
}

/// Cores by number, written as a cpuset list.
struct Cores(Vec<u32>);

/// Why cores were not reserved, or not given back.
#[derive(Clone, Copy)]
enum Unreserved {
    Unread,
    Alike,
    Unconfined,
    Unreleased,
}

impl Unreserved {
    fn on(self, host: &str) -> Box<Diagnostic> {
        let msg = match self {
            Self::Unread => Msg::new().text("cannot read the core capacities of ").code(host),
            Self::Alike => Msg::new().text("every core of ").code(host).text(" is alike: none is fastest"),
            Self::Unconfined => Msg::new().text("cannot confine the services of ").code(host),
            Self::Unreleased => Msg::new().text("cannot give back the cores of the services of ").code(host),
        };
        Box::new(Diagnostic::error(UNRESERVED, msg))
    }
}

impl Reservation {
    /// Reserves the fastest cores of `host`.
    pub(super) fn make(host: &str) -> Result<Self, Box<Diagnostic>> {
        let listing = Self::ssh(host, CAPACITIES.into_iter().chain([CAPACITY_FILES]), Unreserved::Unread)?;
        let capacities = Self::capacities(&listing).ok_or_else(|| Unreserved::Unread.on(host))?;
        let top = capacities.iter().map(|&(_, capacity)| capacity).max().unwrap_or_default();
        let (fastest, others): (Vec<_>, Vec<_>) =
            capacities.into_iter().partition(|&(_, capacity)| capacity == top);
        if others.is_empty() {
            return Err(Unreserved::Alike.on(host));
        }
        let before = Self::ssh(host, SHOW.into_iter().chain([ALLOWED, SERVICES]), Unreserved::Unconfined)?;
        let reservation =
            Self { host: host.to_owned(), fastest: Cores::of(fastest), before: before.trim().to_owned() };
        reservation.set(&Cores::of(others).to_string(), Unreserved::Unconfined)?;
        Ok(reservation)
    }

    /// The words moving the remote shell onto the cores reserved.
    pub(super) fn pinning(&self) -> impl Iterator<Item = Arg> {
        let cores = Arg::Text(Symbol::intern(&self.fastest.to_string()));
        PIN.into_iter().map(Arg::from).chain([cores]).chain(PINNED.map(Arg::from))
    }

    /// Gives the services back the cores they had.
    pub(super) fn release(self) -> Result<(), Box<Diagnostic>> {
        self.set(&self.before, Unreserved::Unreleased)
    }

    /// Confines the services to `cores`, a cpuset list; an empty one lets them run anywhere.
    fn set(&self, cores: &str, failed: Unreserved) -> Result<(), Box<Diagnostic>> {
        let property = format!("{ALLOWED}={cores}");
        Self::ssh(&self.host, SET.into_iter().chain([SERVICES, property.as_str()]), failed).map(drop)
    }

    /// Runs `words` on `host`, returning what it printed.
    fn ssh<'w>(
        host: &str,
        words: impl Iterator<Item = &'w str>,
        failed: Unreserved,
    ) -> Result<String, Box<Diagnostic>> {
        let mut args = vec![Arg::Text(Symbol::intern(host))];
        args.extend(words.map(|w| Arg::Text(Symbol::intern(w))));
        let out = Mirror::call(Program::Ssh, args, Streams::Captured, *failed.on(host))?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Each core's number and capacity, from the lines of [`CAPACITIES`].
    pub(crate) fn capacities(listing: &str) -> Option<Vec<(u32, u32)>> {
        let core = |line: &str| {
            let (path, capacity) = line.split_once(':')?;
            let name = Path::new(path).parent()?.file_name()?.to_str()?;
            Some((name.strip_prefix(CORE)?.parse().ok()?, capacity.trim().parse().ok()?))
        };
        let cores: Option<Vec<_>> = listing.lines().map(core).collect();
        cores.filter(|cores| !cores.is_empty())
    }
}

impl Cores {
    fn of(capacities: Vec<(u32, u32)>) -> Self {
        let mut cores: Vec<u32> = capacities.into_iter().map(|(core, _)| core).collect();
        cores.sort_unstable();
        Self(cores)
    }
}

impl fmt::Display for Cores {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (at, core) in self.0.iter().enumerate() {
            if at > 0 {
                f.write_str(",")?;
            }
            write!(f, "{core}")?;
        }
        Ok(())
    }
}

impl Record for Reservation {
    fn kind(&self) -> &'static str {
        "reserved"
    }

    fn text(&self) -> Text {
        Text::from("cores ")
            .styled(Style::Literal, self.fastest.to_string())
            .plain(" of ")
            .styled(Style::Strong, self.host.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("host", self.host.clone()).with("cores", self.fastest.0.clone())
    }
}

impl Status for Reservation {
    fn verb(&self) -> Verb {
        Verb::good("Reserved")
    }
}
