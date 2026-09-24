//! `vun tool bind` (spec 07, section 5): bindings from a C header check cleanly, and Vun lays
//! their structs out as the C compiler does.

use crate::common::Fixture;
use vun_bind::{Bytes, Reason, Unsupported};

const HEADER: &str = "\
#include <stddef.h>
#include <stdint.h>
#define LIMIT (0x10U)
typedef struct point { int32_t x; long y; } point;
typedef struct { uint8_t r, g, b; } color_t;
struct stat { size_t size; };
int stat(const char *path, struct stat *out);
typedef void (*callback)(void *user, int code);
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
    let bindings = match vun_bind::bind(&header, &["--target=x86_64-linux-gnu".to_owned()], &reserved) {
        Ok(bindings) => bindings,
        // Machines without libclang cannot run this; the command reports the same absence.
        Err(vun_bind::Failure::Missing(_)) => return,
        Err(failure) => panic!("{failure:?}"),
    };
    let reasons: Vec<_> = bindings.skipped.iter().map(|s| (s.name.as_str(), &s.reason)).collect();
    assert_eq!(
        reasons,
        [
            ("value", &Reason::Bytes(Bytes::Union)),
            ("log_line", &Reason::Variadic),
            ("frame_size", &Reason::Unsupported(Unsupported::Opaque("frame".to_owned()))),
        ]
    );
    // Sizes as C computes them for x86_64 Linux, which the layout of each binding must equal.
    let sizes = "
point_size: Int(std.meta.size(point)..=std.meta.size(point)) = 16;
color_size: Int(std.meta.size(color_t)..=std.meta.size(color_t)) = 3;
frame_size_is: Int(std.meta.size(frame)..=std.meta.size(frame)) = 40;
";
    let file = fixture.root.write("c/demo.vun", [bindings.source.as_str(), sizes].concat());
    let found = fixture.reported(&file);
    assert!(found.is_empty(), "{found:?}\n{}", bindings.source);
    // A keyword is escaped, a name every file sees is kept apart.
    assert!(bindings.source.contains("\\match: unsafe (type_: "), "{}", bindings.source);
    assert!(bindings.source.contains("test_: "), "{}", bindings.source);
}
