//! The handles of LIR entities: each table of a unit or a function is an `IndexVec` addressed by
//! its own index.

use vun_index::index;

index! {
    /// A function defined in the unit.
    pub struct FuncId;
    /// A data object defined in the unit.
    pub struct DataId;
    /// A symbol the unit uses and the linker provides.
    pub struct ImportId;
    /// An SSA value of a function.
    pub struct Value;
    /// A basic block of a function.
    pub struct Block;
    /// An instruction of a function.
    pub struct Inst;
    /// A stack slot of a function.
    pub struct Slot;
}
