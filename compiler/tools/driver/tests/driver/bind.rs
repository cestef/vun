//! `vun util ffi bind` (spec 07, section 5): bindings from a C header check cleanly, and Vun lays
//! their structs out as the C compiler does.

use crate::common::{Fixture, built_component};
use vun_binder::{Binder, Bytes, Failure, Glob, Library, Reason, Request, Unsupported};
use vun_driver::{Components, Provider, Registry};

const HEADER: &str = "\
#include <stddef.h>
#include <stdint.h>
#define LIMIT (0x10U)
typedef struct point { int32_t x; long y; } point;
typedef struct { uint8_t r, g, b; } color_t;
struct stat { size_t size; };
int stat(const char *path, struct stat *out);
typedef void (*callback)(void *user, int code);
typedef void (*thunk)(void);
void run_thunk(void);
enum mode { MODE_READ = 1, MODE_WRITE = 2 };
struct handle;
struct handle *open_handle(const char *name, enum mode m);
int log_line(const char *fmt, ...);
union value { int i; float f; };
struct frame { union value v; double samples[3]; callback on_done; };
size_t frame_size(struct frame f);
int match(int type, int test);
";

#[test]
fn bindings_check_and_lay_structs_out_as_c_does() {
    let fixture = Fixture::new("bind");
    let header = fixture.root.write("c/demo.h", HEADER);
    let mut session = fixture.session();
    let reserved = session.names_seen_in(&fixture.root.join("c"));
    let request = Request::new(header, vec!["--target=x86_64-linux-gnu".to_owned()], reserved);
    let bindings = match vun_bind::CBinder.bind(&request) {
        Ok(bindings) => bindings,
        // Machines without libclang cannot run this; the command reports the same absence.
        Err(Failure::Missing(_)) => return,
        Err(failure) => panic!("{failure:?}"),
    };
    let reasons: Vec<_> = bindings.skipped.iter().map(|s| (s.name.as_str(), &s.reason)).collect();
    assert_eq!(
        reasons,
        [
            ("value", &Reason::Bytes(Bytes::Union)),
            ("frame_size", &Reason::Unsupported(Unsupported::Opaque("Frame".to_owned()))),
        ]
    );
    // Sizes as C computes them for x86_64 Linux, which the layout of each binding must equal.
    let sizes = "
point_size: Int(std.meta.size(Point)..=std.meta.size(Point)) = 16;
color_size: Int(std.meta.size(ColorT)..=std.meta.size(ColorT)) = 3;
frame_size_is: Int(std.meta.size(Frame)..=std.meta.size(Frame)) = 40;
";
    let file = fixture.root.write("c/demo.vun", [bindings.source.as_str(), sizes].concat());
    let found = fixture.reported(&file);
    assert!(found.is_empty(), "{found:?}\n{}", bindings.source);
    // Names take the case of what they bind; the linker's keep C's through `link.symbol`.
    assert!(bindings.source.contains("mode_read: Mode = 1"), "{}", bindings.source);
    // The text C holds is bytes, so `char` is `u8`; `signed char` is not.
    assert!(
        bindings.source.contains("open_handle: unsafe (name: ?*lang.int.unsigned(8)"),
        "{}",
        bindings.source
    );
    // A function type taking and giving nothing is not the unit type.
    assert!(bindings.source.contains("Thunk: type = ?#[lang.abi.c] () -> ()"), "{}", bindings.source);
    assert!(bindings.source.contains("run_thunk: unsafe () -> () = _"), "{}", bindings.source);
    // A variadic function takes the arguments past its parameters as a pack, its type inferred.
    assert!(bindings.source.contains("log_line: unsafe ($P: type, fmt: "), "{}", bindings.source);
    assert!(bindings.source.contains("..args: P"), "{}", bindings.source);
    // A keyword is escaped, a name every file sees is kept apart.
    assert!(bindings.source.contains("\\match: unsafe (type_: "), "{}", bindings.source);
    assert!(bindings.source.contains("test_: "), "{}", bindings.source);
}

#[test]
fn a_binder_loaded_from_a_library_binds_as_the_linked_one_does() {
    let fixture = Fixture::new("bind-component");
    let header = fixture.root.write("c/demo.h", HEADER);
    let request = Request::new(header, vec!["--target=x86_64-linux-gnu".to_owned()], Vec::new());
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component("vun-component-binder-c")));
    let loaded = registry.binders.get("c").expect("the library serves a binder");
    assert!(matches!(loaded.from, Provider::Loaded(_)));
    assert_eq!(loaded.item.bind(&request), vun_bind::CBinder.bind(&request));
}

/// A library's header in one directory, which includes the one holding what it builds on.
const DEPENDENCY: &str = "\
#include <stdint.h>
typedef struct _Window Window;
typedef enum { FILL, START, END } Align;
typedef struct { int32_t x, y; } Size;
Window *window_new(Size *size, Align align);
";

const FRONT: &str = "\
#include \"../dep/dep.h\"
#define LIB_VERSION \"1.2\"
#define LIB_PI 3.14159
#define LIB_SHIFT (1 << 4 | 2)
#define LIB_LIMIT (LIB_SHIFT * 2)
void window_set_title(Window *w, const char *title);
void window_close(Window *w);
int log_to(Window *w, const char *fmt, ...);
void other_thing(int unrelated);
";

#[test]
fn a_request_binds_the_directories_it_names_and_only_what_it_asks_for() {
    let fixture = Fixture::new("bind-scope");
    fixture.root.write("dep/dep.h", DEPENDENCY);
    let header = fixture.root.write("front/front.h", FRONT);
    let mut session = fixture.session();
    let reserved = session.names_seen_in(&fixture.root.join("out"));
    let request = Request {
        scope: vec![fixture.root.join("dep")],
        only: vec![Glob::new("window_*"), Glob::new("LIB_*"), Glob::new("log_*")],
        libraries: vec![
            Library::named("front"),
            Library::named(&format!("deep={}", fixture.root.join("dep").display())),
        ],
        ..Request::new(header, vec!["--target=x86_64-linux-gnu".to_owned()], reserved)
    };
    let bindings = match vun_bind::CBinder.bind(&request) {
        Ok(bindings) => bindings,
        Err(Failure::Missing(_)) => return,
        Err(failure) => panic!("{failure:?}"),
    };
    let text = &bindings.source;
    // What a name asks for brings what it is written with, from the other directory too, an
    // opaque struct included, and what it does not ask for stays out.
    for wanted in ["window_new", "window_set_title", "Window", "Size", "Align"] {
        assert!(text.contains(wanted), "{wanted}\n{text}");
    }
    assert!(!text.contains("other_thing"), "{text}");
    // Each import names the library its header is linked from: of those naming it, the one under
    // the deepest directory.
    assert!(
        text.contains("link.symbol(\"window_set_title\"), lang.abi.c, lang.link.library(\"front\")"),
        "{text}"
    );
    assert!(text.contains("link.symbol(\"window_new\"), lang.abi.c, lang.link.library(\"deep\")"), "{text}");
    // Macros bind as what they are worth, naming each other.
    assert!(text.contains("lib_version := c\"1.2\""), "{text}");
    assert!(text.contains("lib_pi := 3.14159"), "{text}");
    assert!(text.contains("lib_shift := 18"), "{text}");
    assert!(text.contains("lib_limit := 36"), "{text}");
    assert_eq!(bindings.skipped, []);
    let file = fixture.root.write("out/front.vun", text);
    let found = fixture.reported(&file);
    assert!(found.is_empty(), "{found:?}\n{text}");
}
