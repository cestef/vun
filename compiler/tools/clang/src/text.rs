//! libclang's strings, copied out and released.

use std::ffi::CStr;

use clang_sys::{CXString, clang_disposeString, clang_getCString};

/// The text of `s`, which is released.
pub(crate) fn take(s: CXString) -> String {
    // SAFETY: `s` came from libclang and is released exactly once, after its text is copied; a
    // null pointer is an empty string.
    unsafe {
        let ptr = clang_getCString(s);
        let text =
            if ptr.is_null() { String::new() } else { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        clang_disposeString(s);
        text
    }
}
