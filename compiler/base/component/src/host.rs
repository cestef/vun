//! The host's side: opening a component, calling it, answering what it asks.

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use vun_trace::tracing::debug;
use vun_trace::{Activity, Timer, Timings};

use crate::abi::{ABI, Beat, Descriptor, Host, Owned, Relay, Slice, Status};
use crate::{Asked, Fault, Interface, RELEASE, Wire};

/// A component opened: its descriptor, valid for the process.
#[derive(Clone)]
pub struct Component {
    descriptor: &'static Descriptor,
    /// The library it is from, if it was opened from one.
    path: Option<Arc<Path>>,
}

impl Component {
    /// The component a descriptor of this process describes.
    ///
    /// # Errors
    /// Returns why it is not one this `vun` can call.
    pub fn within(descriptor: &'static Descriptor) -> Result<Self, Fault> {
        Self::checked(descriptor, None)
    }

    /// Opens the library at `path`, whose hash the caller has checked against the toolchain's
    /// manifest, and keeps it mapped for the process: code that may still be running is never
    /// unloaded.
    ///
    /// # Errors
    /// Returns why it does not open, or is not a component of this release.
    #[cfg(not(target_family = "wasm"))]
    pub fn open(path: &Path) -> Result<Self, Fault> {
        let open =
            |source: libloading::Error| Fault::Open { path: path.to_owned(), source: Box::new(source) };
        // SAFETY: loading runs the library's initializers, which are those of a component the
        // toolchain's signed manifest names (the caller's check).
        let library = unsafe { libloading::Library::new(path) }.map_err(open)?;
        // SAFETY: the entry is `fn() -> &'static Descriptor`, as every component exports.
        let entry: libloading::Symbol<'_, extern "C" fn() -> *const Descriptor> =
            unsafe { library.get(crate::abi::ENTRY) }.map_err(open)?;
        let descriptor = entry();
        // Code that may run is never unloaded: the library stays mapped as long as the process.
        std::mem::forget(library);
        // SAFETY: a descriptor begins with its layout's number, which is read before any other
        // field; the library is mapped for the process.
        let found = unsafe { descriptor.cast::<u32>().read() };
        if found != ABI {
            return Err(Fault::Abi { found, expected: ABI });
        }
        // SAFETY: of the layout this build knows, as just checked.
        Self::checked(unsafe { &*descriptor }, Some(path.into()))
    }

    /// Where a platform loads no libraries, none opens.
    ///
    /// # Errors
    /// Always: [`Fault::Open`].
    #[cfg(target_family = "wasm")]
    pub fn open(path: &Path) -> Result<Self, Fault> {
        let unsupported = std::io::Error::from(std::io::ErrorKind::Unsupported);
        Err(Fault::Open { path: path.to_owned(), source: Box::new(unsupported) })
    }

    fn checked(descriptor: &'static Descriptor, path: Option<Arc<Path>>) -> Result<Self, Fault> {
        if descriptor.abi != ABI {
            return Err(Fault::Abi { found: descriptor.abi, expected: ABI });
        }
        // SAFETY: a static string of the component.
        let found = String::from_utf8_lossy(unsafe { descriptor.release.bytes() }).into_owned();
        if found != RELEASE {
            return Err(Fault::Release { found, expected: RELEASE });
        }
        Ok(Self { descriptor, path })
    }

    /// The library it is from.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.path.as_deref().map(Path::to_path_buf)
    }

    /// The kind of component it is, as the interface that serves it names it.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        // SAFETY: a static string of the component, which stays mapped.
        std::str::from_utf8(unsafe { self.descriptor.kind.bytes() }).unwrap_or_default()
    }

    /// Calls the component of interface `I`, answering what it asks through `ask`.
    ///
    /// # Errors
    /// Returns why there is no reply: another kind of component, a panic inside, a malformed
    /// message.
    pub fn call<I: Interface>(&self, call: &I::Call<'_>, ask: Asked<'_, I>) -> Result<I::Reply, Fault> {
        if self.kind() != I::KIND.0 {
            return Err(Fault::Kind { found: self.kind().to_owned(), expected: I::KIND });
        }
        let started = Instant::now();
        let _timer = Activity::component(I::KIND.0).start();
        let request = Wire::seal(call);
        // A component linked in shares the host's table: nothing to relay.
        let relay = (Timings::enabled() && self.path.is_some()).then_some(Self::relay as Relay);
        let host =
            Host { context: (&raw const ask).cast(), ask: Self::answer::<I>, free: Owned::release, relay };
        let mut out = Owned::EMPTY;
        // SAFETY: the component's own function, given a host that lives through the call, a live
        // request and an `out`.
        let status = unsafe { (self.descriptor.call)(&raw const host, Slice::of(&request), &raw mut out) };
        // SAFETY: filled by the component's `call`, whose `free` it is.
        let reply = unsafe { out.copied(self.descriptor.free) };
        debug!(
            target: "vun_component",
            kind = ?I::KIND, sent = request.len(), received = reply.len(), ?status,
            micros = started.elapsed().as_micros(), library = ?self.path,
            "called a component",
        );
        match status {
            Status::Done => Wire::open(&reply).map_err(Fault::Malformed),
            Status::Panicked => Err(Fault::Panicked(String::from_utf8_lossy(&reply).into_owned())),
            Status::Malformed => Err(Fault::Unread(String::from_utf8_lossy(&reply).into_owned())),
        }
    }

    /// What the component's asks of the host reach: decodes the question, calls `ask`, writes the
    /// answer. A panic of the host's is reported to the component, not unwound through it.
    unsafe extern "C" fn answer<I: Interface>(
        context: *const c_void,
        input: Slice,
        out: *mut Owned,
    ) -> Status {
        let _timer = Activity::component("ask").start();
        // SAFETY: `context` is the `ask` of the call in progress, which lives through it.
        let ask = unsafe { *context.cast::<Asked<'_, I>>() };
        // SAFETY: a live slice, by the component.
        let request = unsafe { input.bytes() };
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            Wire::open::<I::Ask>(request).map(|question| Wire::seal(&ask(question)))
        }));
        let (status, bytes) = match outcome {
            Ok(Ok(answer)) => (Status::Done, answer),
            Ok(Err(malformed)) => (Status::Malformed, malformed.to_string().into_bytes()),
            Err(payload) => (Status::Panicked, Fault::text_of(&*payload).into_bytes()),
        };
        // SAFETY: writable, by the component.
        unsafe { out.write(Owned::new(bytes)) };
        status
    }

    /// Times in the host's table what a component times: each beat of the library is the start or
    /// the end of an activity here, nested under the call.
    unsafe extern "C" fn relay(beat: Beat, kind: u32, name: Slice) {
        thread_local! {
            /// The activities the component has begun on this thread, innermost last.
            static BEGUN: RefCell<Vec<Option<Timer>>> = const { RefCell::new(Vec::new()) };
        }
        BEGUN.with_borrow_mut(|begun| match beat {
            Beat::Begin => {
                // SAFETY: a name of the library, which stays mapped for the process.
                let name = std::str::from_utf8(unsafe { name.bytes::<'static>() }).ok();
                let kind = vun_trace::Kind::numbered(kind);
                begun.push(kind.zip(name).map(|(kind, name)| Activity { kind, name }.start_here()));
            }
            Beat::End => drop(begun.pop()),
        });
    }
}
