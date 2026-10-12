//! Types as the target's C compiler sees them: sizes and alignments come from clang, for the
//! target the flags name.

use std::marker::PhantomData;

use clang_sys::{
    CXTranslationUnit, CXType, CXType_Attributed, CXType_Bool, CXType_Char_S, CXType_Char_U,
    CXType_ConstantArray, CXType_Double, CXType_Elaborated, CXType_Enum, CXType_Float, CXType_Float16,
    CXType_FunctionNoProto, CXType_FunctionProto, CXType_Half, CXType_IncompleteArray, CXType_Int,
    CXType_Int128, CXType_Long, CXType_LongDouble, CXType_LongLong, CXType_Pointer, CXType_Record,
    CXType_SChar, CXType_Short, CXType_Typedef, CXType_UChar, CXType_UInt, CXType_UInt128, CXType_ULong,
    CXType_ULongLong, CXType_UShort, CXType_Void, CXTypeNullability_NonNull, clang_Type_getAlignOf,
    clang_Type_getModifiedType, clang_Type_getNamedType, clang_Type_getNullability, clang_Type_getSizeOf,
    clang_getArgType, clang_getArrayElementType, clang_getArraySize, clang_getCanonicalType,
    clang_getNumArgTypes, clang_getPointeeType, clang_getResultType, clang_getTypeDeclaration,
    clang_getTypeSpelling, clang_isConstQualifiedType, clang_isFunctionTypeVariadic,
};

use crate::{Cursor, text};

/// The kinds of type bindings tell apart; any other is `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Void,
    Bool,
    /// Plain `char`, the byte of text, which a target makes signed or unsigned as it likes.
    Char,
    /// An integer, `signed` or not, of the size [`Type::size`] gives: `signed char`,
    /// `unsigned char`, `short`, `int`, `long`, `long long`, `__int128`.
    Integer {
        signed: bool,
    },
    Float,
    Pointer,
    Record,
    Enum,
    Typedef,
    Array,
    /// `T[]`, as a parameter or a trailing member.
    IncompleteArray,
    Function,
    Other,
}

#[derive(Clone, Copy)]
pub struct Type<'u> {
    raw: CXType,
    /// Whether the header says a pointer of this type is never null, as `_Nonnull` does.
    nonnull: bool,
    unit: CXTranslationUnit,
    _unit: PhantomData<&'u ()>,
}

impl<'u> Type<'u> {
    /// The type `raw` is, as the type it names when it is written with its tag, as `struct point`
    /// is: clang before 22 reports those apart, as elaborated types, and later ones do not. The
    /// attributes written on it (`_Nonnull`) are read, and it is the type they modify.
    pub(crate) fn new(raw: CXType, unit: CXTranslationUnit) -> Self {
        let mut raw = raw;
        let mut nonnull = false;
        loop {
            match raw.kind {
                CXType_Elaborated => raw = unsafe { clang_Type_getNamedType(raw) },
                CXType_Attributed => {
                    nonnull |= unsafe { clang_Type_getNullability(raw) } == CXTypeNullability_NonNull;
                    raw = unsafe { clang_Type_getModifiedType(raw) };
                }
                _ => break,
            }
        }
        Self { raw, nonnull, unit, _unit: PhantomData }
    }

    fn wrap(&self, raw: CXType) -> Self {
        Self::new(raw, self.unit)
    }

    #[must_use]
    pub fn kind(&self) -> TypeKind {
        match self.raw.kind {
            CXType_Void => TypeKind::Void,
            CXType_Bool => TypeKind::Bool,
            CXType_Char_S | CXType_Char_U => TypeKind::Char,
            CXType_SChar | CXType_Short | CXType_Int | CXType_Long | CXType_LongLong | CXType_Int128 => {
                TypeKind::Integer { signed: true }
            }
            CXType_UChar | CXType_UShort | CXType_UInt | CXType_ULong | CXType_ULongLong | CXType_UInt128 => {
                TypeKind::Integer { signed: false }
            }
            CXType_Float | CXType_Double | CXType_LongDouble | CXType_Half | CXType_Float16 => {
                TypeKind::Float
            }
            CXType_Pointer => TypeKind::Pointer,
            CXType_Record => TypeKind::Record,
            CXType_Enum => TypeKind::Enum,
            CXType_Typedef => TypeKind::Typedef,
            CXType_ConstantArray => TypeKind::Array,
            CXType_IncompleteArray => TypeKind::IncompleteArray,
            CXType_FunctionProto | CXType_FunctionNoProto => TypeKind::Function,
            _ => TypeKind::Other,
        }
    }

    /// As C writes it, qualifiers included.
    #[must_use]
    pub fn spelling(&self) -> String {
        // SAFETY: every call below reads a type of a unit alive for `'u`.
        text::take(unsafe { clang_getTypeSpelling(self.raw) })
    }

    /// Its size in bytes on the target; `None` for an incomplete type or `void`.
    #[must_use]
    pub fn size(&self) -> Option<u64> {
        u64::try_from(unsafe { clang_Type_getSizeOf(self.raw) }).ok()
    }

    /// Its alignment in bytes on the target.
    #[must_use]
    pub fn align(&self) -> Option<u64> {
        u64::try_from(unsafe { clang_Type_getAlignOf(self.raw) }).ok()
    }

    #[must_use]
    pub fn pointee(&self) -> Self {
        self.wrap(unsafe { clang_getPointeeType(self.raw) })
    }

    /// Through every typedef.
    #[must_use]
    pub fn canonical(&self) -> Self {
        self.wrap(unsafe { clang_getCanonicalType(self.raw) })
    }

    /// The declaration of a record, enum or typedef type.
    #[must_use]
    pub fn declaration(&self) -> Cursor<'u> {
        Cursor::new(unsafe { clang_getTypeDeclaration(self.raw) }, self.unit)
    }

    /// An array's item type.
    #[must_use]
    pub fn element(&self) -> Self {
        self.wrap(unsafe { clang_getArrayElementType(self.raw) })
    }

    /// A constant array's length.
    #[must_use]
    pub fn array_len(&self) -> Option<u64> {
        u64::try_from(unsafe { clang_getArraySize(self.raw) }).ok()
    }

    /// A function type's result.
    #[must_use]
    pub fn result(&self) -> Self {
        self.wrap(unsafe { clang_getResultType(self.raw) })
    }

    /// A function type's parameter types.
    #[must_use]
    pub fn params(&self) -> Vec<Self> {
        let n = u32::try_from(unsafe { clang_getNumArgTypes(self.raw) }).unwrap_or(0);
        (0..n).map(|i| self.wrap(unsafe { clang_getArgType(self.raw, i) })).collect()
    }

    /// Whether it is `const`: what C code only reads.
    #[must_use]
    pub fn is_const(&self) -> bool {
        unsafe { clang_isConstQualifiedType(self.raw) != 0 }
    }

    /// Whether the header says it is never null, as `_Nonnull` does of a pointer.
    #[must_use]
    pub fn is_nonnull(&self) -> bool {
        self.nonnull
    }

    #[must_use]
    pub fn is_variadic(&self) -> bool {
        unsafe { clang_isFunctionTypeVariadic(self.raw) != 0 }
    }
}
