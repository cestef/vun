//! Cursors: the declarations and macros of a unit, with what bindings read of each.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::path::PathBuf;

use clang_sys::{
    CX_SC_Static, CXChildVisit_Continue, CXChildVisitResult, CXClientData, CXCursor,
    CXCursor_EnumConstantDecl, CXCursor_EnumDecl, CXCursor_FieldDecl, CXCursor_FunctionDecl,
    CXCursor_MacroDefinition, CXCursor_ParmDecl, CXCursor_StructDecl, CXCursor_TypedefDecl,
    CXCursor_UnionDecl, CXCursor_VarDecl, CXFile, CXToken, CXTranslationUnit, clang_Cursor_getArgument,
    clang_Cursor_getNumArguments, clang_Cursor_getStorageClass, clang_Cursor_isAnonymousRecordDecl,
    clang_Cursor_isBitField, clang_Cursor_isMacroFunctionLike, clang_Cursor_isVariadic, clang_disposeTokens,
    clang_getCursorExtent, clang_getCursorKind, clang_getCursorLocation, clang_getCursorResultType,
    clang_getCursorSpelling, clang_getCursorType, clang_getEnumConstantDeclValue,
    clang_getEnumDeclIntegerType, clang_getExpansionLocation, clang_getFileName, clang_getTokenSpelling,
    clang_getTypedefDeclUnderlyingType, clang_isCursorDefinition, clang_tokenize, clang_visitChildren,
};

use crate::{Type, text};

/// A place in a unit, `file` `None` for what the compiler declares itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: Option<PathBuf>,
    pub line: u32,
    pub column: u32,
}

/// The kinds of cursor bindings read; any other is `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Function,
    Struct,
    Union,
    Enum,
    EnumConstant,
    Field,
    Typedef,
    Var,
    Param,
    Macro,
    Other,
}

#[derive(Clone, Copy)]
pub struct Cursor<'u> {
    raw: CXCursor,
    unit: CXTranslationUnit,
    _unit: PhantomData<&'u ()>,
}

impl<'u> Cursor<'u> {
    pub(crate) fn new(raw: CXCursor, unit: CXTranslationUnit) -> Self {
        Self { raw, unit, _unit: PhantomData }
    }

    fn wrap(&self, raw: CXCursor) -> Self {
        Self::new(raw, self.unit)
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        // SAFETY: every call below reads a cursor of a unit alive for `'u`.
        match unsafe { clang_getCursorKind(self.raw) } {
            CXCursor_FunctionDecl => Kind::Function,
            CXCursor_StructDecl => Kind::Struct,
            CXCursor_UnionDecl => Kind::Union,
            CXCursor_EnumDecl => Kind::Enum,
            CXCursor_EnumConstantDecl => Kind::EnumConstant,
            CXCursor_FieldDecl => Kind::Field,
            CXCursor_TypedefDecl => Kind::Typedef,
            CXCursor_VarDecl => Kind::Var,
            CXCursor_ParmDecl => Kind::Param,
            CXCursor_MacroDefinition => Kind::Macro,
            _ => Kind::Other,
        }
    }

    /// The declared name; empty for an anonymous one.
    #[must_use]
    pub fn name(&self) -> String {
        text::take(unsafe { clang_getCursorSpelling(self.raw) })
    }

    /// Where it is written, after macro expansion.
    #[must_use]
    pub fn location(&self) -> Location {
        let (mut file, mut line, mut column): (CXFile, u32, u32) = (std::ptr::null_mut(), 0, 0);
        // SAFETY: the out pointers are valid for the call; the offset is not asked for.
        unsafe {
            clang_getExpansionLocation(
                clang_getCursorLocation(self.raw),
                &raw mut file,
                &raw mut line,
                &raw mut column,
                std::ptr::null_mut(),
            );
        }
        let file = (!file.is_null()).then(|| PathBuf::from(text::take(unsafe { clang_getFileName(file) })));
        Location { file, line, column }
    }

    /// Its children, in source order.
    #[must_use]
    pub fn children(&self) -> Vec<Self> {
        extern "C" fn visit(child: CXCursor, _: CXCursor, data: CXClientData) -> CXChildVisitResult {
            // SAFETY: `data` is the vector `children` passes, alive for the whole visit.
            unsafe { (*data.cast::<Vec<CXCursor>>()).push(child) };
            CXChildVisit_Continue
        }
        let mut found: Vec<CXCursor> = Vec::new();
        unsafe { clang_visitChildren(self.raw, visit, (&raw mut found).cast::<c_void>()) };
        found.into_iter().map(|raw| self.wrap(raw)).collect()
    }

    #[must_use]
    pub fn ty(&self) -> Type<'u> {
        Type::new(unsafe { clang_getCursorType(self.raw) }, self.unit)
    }

    /// A function's result type.
    #[must_use]
    pub fn result(&self) -> Type<'u> {
        Type::new(unsafe { clang_getCursorResultType(self.raw) }, self.unit)
    }

    /// A function's parameters, named as declared.
    #[must_use]
    pub fn params(&self) -> Vec<Self> {
        let n = u32::try_from(unsafe { clang_Cursor_getNumArguments(self.raw) }).unwrap_or(0);
        (0..n).map(|i| self.wrap(unsafe { clang_Cursor_getArgument(self.raw, i) })).collect()
    }

    /// Whether a function takes arguments past its parameters, `...`.
    #[must_use]
    pub fn is_variadic(&self) -> bool {
        unsafe { clang_Cursor_isVariadic(self.raw) != 0 }
    }

    /// Whether this declaration is the definition: a struct with its fields.
    #[must_use]
    pub fn is_definition(&self) -> bool {
        unsafe { clang_isCursorDefinition(self.raw) != 0 }
    }

    /// Whether a function or variable is `static`: not linked, so nothing binds it.
    #[must_use]
    pub fn is_static(&self) -> bool {
        unsafe { clang_Cursor_getStorageClass(self.raw) == CX_SC_Static }
    }

    #[must_use]
    pub fn is_bit_field(&self) -> bool {
        unsafe { clang_Cursor_isBitField(self.raw) != 0 }
    }

    /// Whether a struct or union has no name of its own, as one nested in a field.
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        unsafe { clang_Cursor_isAnonymousRecordDecl(self.raw) != 0 }
    }

    /// An enum constant's value.
    #[must_use]
    pub fn enum_value(&self) -> i64 {
        unsafe { clang_getEnumConstantDeclValue(self.raw) }
    }

    /// The integer type an enum's constants have.
    #[must_use]
    pub fn enum_type(&self) -> Type<'u> {
        Type::new(unsafe { clang_getEnumDeclIntegerType(self.raw) }, self.unit)
    }

    /// The type a typedef names.
    #[must_use]
    pub fn underlying(&self) -> Type<'u> {
        Type::new(unsafe { clang_getTypedefDeclUnderlyingType(self.raw) }, self.unit)
    }

    /// Whether a macro takes arguments.
    #[must_use]
    pub fn is_function_like(&self) -> bool {
        unsafe { clang_Cursor_isMacroFunctionLike(self.raw) != 0 }
    }

    /// The tokens it is written with, as spelled: a macro's name, then its replacement.
    #[must_use]
    pub fn tokens(&self) -> Vec<String> {
        let (mut tokens, mut count): (*mut CXToken, u32) = (std::ptr::null_mut(), 0);
        // SAFETY: libclang fills both out pointers; the tokens are released once, after being
        // read, and `count` of them are valid.
        unsafe {
            clang_tokenize(self.unit, clang_getCursorExtent(self.raw), &raw mut tokens, &raw mut count);
            if tokens.is_null() {
                return Vec::new();
            }
            let spelled = std::slice::from_raw_parts(tokens, count as usize)
                .iter()
                .map(|&t| text::take(clang_getTokenSpelling(self.unit, t)))
                .collect();
            clang_disposeTokens(self.unit, tokens, count);
            spelled
        }
    }
}
