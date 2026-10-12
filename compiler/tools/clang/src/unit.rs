//! A translation unit: one header parsed with the build's flags, and what clang reported.

use std::ffi::CString;
use std::marker::PhantomData;
use std::path::Path;

use clang_sys::{
    CXDiagnostic_DisplayColumn, CXDiagnostic_DisplaySourceLocation, CXDiagnostic_DisplaySourceRanges,
    CXDiagnostic_Error, CXDiagnostic_Fatal, CXDiagnostic_Warning, CXIndex, CXTranslationUnit,
    CXTranslationUnit_DetailedPreprocessingRecord, CXTranslationUnit_IncludeAttributedTypes,
    CXTranslationUnit_SkipFunctionBodies, clang_createIndex, clang_disposeDiagnostic, clang_disposeIndex,
    clang_disposeTranslationUnit, clang_formatDiagnostic, clang_getDiagnostic, clang_getDiagnosticSeverity,
    clang_getNumDiagnostics, clang_getTranslationUnitCursor, clang_parseTranslationUnit2,
};

use crate::{Clang, Cursor, text};

/// A parsed header. Macro definitions are kept, so that constants written as macros are read.
pub struct Unit<'c> {
    index: CXIndex,
    raw: CXTranslationUnit,
    _clang: PhantomData<&'c Clang>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Note,
    Warning,
    Error,
}

/// What clang reported while parsing, formatted as clang formats it, with the place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub text: String,
}

/// The flag naming the directory of the headers clang provides, as `-resource-dir <dir>` or
/// `-resource-dir=<dir>`.
const RESOURCE_DIR: &str = "-resource-dir";

impl Clang {
    /// Parses `header` as C with the compile flags `args`, such as `-I`, `-D` and `--target`;
    /// `None` when clang could not start parsing at all. The headers clang provides are found
    /// next to the library, unless `args` name their directory with `-resource-dir`.
    #[must_use]
    pub fn parse(&self, header: &Path, args: &[String]) -> Option<Unit<'_>> {
        let file = CString::new(header.as_os_str().as_encoded_bytes()).ok()?;
        let given = |a: &String| {
            a.strip_prefix(RESOURCE_DIR).is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
        };
        let resource = if args.iter().any(given) {
            None
        } else {
            clang_sys::get_library().and_then(|l| crate::resource::find(l.path()))
        };
        let resource = resource.and_then(|dir| Some([RESOURCE_DIR.to_owned(), dir.to_str()?.to_owned()]));
        let args: Vec<CString> = std::iter::once("-xc".to_owned())
            .chain(resource.into_iter().flatten())
            .chain(args.iter().cloned())
            .map(CString::new)
            .collect::<Result<_, _>>()
            .ok()?;
        let pointers: Vec<*const std::ffi::c_char> = args.iter().map(|a| a.as_ptr()).collect();
        // Attributed types are kept, since `_Nonnull` is read from them.
        let flags = CXTranslationUnit_DetailedPreprocessingRecord
            | CXTranslationUnit_SkipFunctionBodies
            | CXTranslationUnit_IncludeAttributedTypes;
        // SAFETY: the strings outlive the call; the index is released by `Unit`'s drop, or here
        // when parsing fails.
        unsafe {
            let index = clang_createIndex(0, 0);
            let mut unit: CXTranslationUnit = std::ptr::null_mut();
            let code = clang_parseTranslationUnit2(
                index,
                file.as_ptr(),
                pointers.as_ptr(),
                i32::try_from(pointers.len()).ok()?,
                std::ptr::null_mut(),
                0,
                flags,
                &raw mut unit,
            );
            if code != 0 || unit.is_null() {
                clang_disposeIndex(index);
                return None;
            }
            Some(Unit { index, raw: unit, _clang: PhantomData })
        }
    }
}

impl Unit<'_> {
    /// The unit's root, whose children are its top-level declarations and macros.
    #[must_use]
    pub fn root(&self) -> Cursor<'_> {
        // SAFETY: the unit is alive for the cursor's lifetime.
        Cursor::new(unsafe { clang_getTranslationUnitCursor(self.raw) }, self.raw)
    }

    /// What clang reported, in order.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        // SAFETY: each diagnostic is released after it is read.
        unsafe {
            (0..clang_getNumDiagnostics(self.raw))
                .map(|i| {
                    let d = clang_getDiagnostic(self.raw, i);
                    let severity = match clang_getDiagnosticSeverity(d) {
                        CXDiagnostic_Error | CXDiagnostic_Fatal => Severity::Error,
                        CXDiagnostic_Warning => Severity::Warning,
                        _ => Severity::Note,
                    };
                    let shown = CXDiagnostic_DisplaySourceLocation
                        | CXDiagnostic_DisplayColumn
                        | CXDiagnostic_DisplaySourceRanges;
                    let text = text::take(clang_formatDiagnostic(d, shown));
                    clang_disposeDiagnostic(d);
                    Diagnostic { severity, text }
                })
                .collect()
        }
    }
}

impl Drop for Unit<'_> {
    fn drop(&mut self) {
        // SAFETY: both were made by `parse` and are released once.
        unsafe {
            clang_disposeTranslationUnit(self.raw);
            clang_disposeIndex(self.index);
        }
    }
}
