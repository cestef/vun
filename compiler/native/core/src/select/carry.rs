//! Values between memory and the carriers a convention passes them in, on load and store
//! architectures: a general register loaded or stored in the pieces an access takes, a
//! floating-point register at the float's width, and bytes copied between two addresses. What
//! differs between architectures, the accesses and shifts, are the hooks of [`Carries`].

use vun_asm::Width;
use vun_codegen::snippet::Emits;
use vun_codegen::{Cx, pieces};

use super::{Carried, Selects};
use vun_abi::Carrier;

/// Which way an access moves bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Load,
    Store,
}

/// What a load and store architecture's accesses of carriers are written with.
pub trait Carries: Selects + Emits<Size: Width> {
    /// The sizes an access of a general register moves, widest first.
    const PIECES: &'static [<Self as Emits>::Size];
    /// The register an offset too far for one instruction is built in.
    const FAR: Self::Reg;

    /// The bytes a piece of `size` moves.
    fn piece_bytes(size: <Self as Emits>::Size) -> u64;

    /// Moves `reg`, of `size`, to or from memory `disp` past `base`.
    fn access(
        cx: &mut Cx<'_, Self>,
        way: Way,
        size: <Self as Emits>::Size,
        reg: Self::Reg,
        base: Self::Reg,
        disp: i64,
    );

    /// Whether one access `way` of `size` reaches `disp` past `base`.
    fn reaches(way: Way, size: <Self as Emits>::Size, reg: Self::Reg, base: Self::Reg, disp: i64) -> bool;

    /// Moves the floating-point register `x`, `bytes` wide, to or from memory `disp` past `base`.
    fn float_access(cx: &mut Cx<'_, Self>, way: Way, x: Self::Float, bytes: u64, base: Self::Reg, disp: i64);

    /// `r = 0`.
    fn clear(cx: &mut Cx<'_, Self>, r: Self::Reg);

    /// `dst |= piece << 8 · at`.
    fn or_piece(cx: &mut Cx<'_, Self>, dst: Self::Reg, piece: Self::Reg, at: i64);

    /// `dst = src >> 8 · at`.
    fn piece_of(cx: &mut Cx<'_, Self>, dst: Self::Reg, src: Self::Reg, at: i64);

    /// Loads the `bytes` bytes `disp` past `base` into `carrier`.
    fn load_part(cx: &mut Cx<'_, Self>, carrier: Carried<Self>, base: Self::Reg, disp: i64, bytes: u64) {
        match carrier {
            Carrier::Int(r) => Self::load_bytes(cx, r, base, disp, bytes),
            Carrier::Float(x) => Self::float_access(cx, Way::Load, x, bytes, base, disp),
        }
    }

    /// Stores the low `bytes` bytes `carrier` holds `disp` past `base`.
    fn store_part(cx: &mut Cx<'_, Self>, base: Self::Reg, disp: i64, carrier: Carried<Self>, bytes: u64) {
        match carrier {
            Carrier::Int(r) => Self::store_bytes(cx, base, disp, r, bytes),
            Carrier::Float(x) => Self::float_access(cx, Way::Store, x, bytes, base, disp),
        }
    }

    /// Loads `bytes` bytes, a register's at most, `disp` past `base` into `dst`, in pieces a load
    /// can take, gathered through the framework's temporary register.
    fn load_bytes(cx: &mut Cx<'_, Self>, dst: Self::Reg, base: Self::Reg, disp: i64, bytes: u64) {
        if let Some(size) = <<Self as Emits>::Size as Width>::of_bytes(bytes) {
            Self::access(cx, Way::Load, size, dst, base, disp);
            return;
        }
        Self::clear(cx, dst);
        for (size, at) in pieces(bytes, Self::PIECES, Self::piece_bytes) {
            Self::access(cx, Way::Load, size, Self::TEMP, base, disp + at);
            Self::or_piece(cx, dst, Self::TEMP, at);
        }
    }

    /// Stores the low `bytes` bytes of `src`, a register's at most, `disp` past `base`.
    fn store_bytes(cx: &mut Cx<'_, Self>, base: Self::Reg, disp: i64, src: Self::Reg, bytes: u64) {
        if let Some(size) = <<Self as Emits>::Size as Width>::of_bytes(bytes) {
            Self::access(cx, Way::Store, size, src, base, disp);
            return;
        }
        for (size, at) in pieces(bytes, Self::PIECES, Self::piece_bytes) {
            let piece = if at == 0 {
                src
            } else {
                Self::piece_of(cx, Self::TEMP, src, at);
                Self::TEMP
            };
            Self::access(cx, Way::Store, size, piece, base, disp + at);
        }
    }

    /// Copies `bytes` bytes from `from` to `to`, both registers holding addresses, through `via`,
    /// largest pieces first. A piece past what an access reaches moves both addresses up to it
    /// rather than building its offset, so no register but these is written, and `via` may be
    /// the far register; both addresses are as they were after.
    fn copy_bytes(cx: &mut Cx<'_, Self>, to: Self::Reg, from: Self::Reg, bytes: u64, via: Self::Reg) {
        let mut moved = 0;
        for (size, at) in pieces(bytes, Self::PIECES, Self::piece_bytes) {
            let disp = at - moved;
            if !Self::reaches(Way::Load, size, via, from, disp)
                || !Self::reaches(Way::Store, size, via, to, disp)
            {
                Self::offset(cx, from, from, at - moved);
                Self::offset(cx, to, to, at - moved);
                moved = at;
            }
            Self::access(cx, Way::Load, size, via, from, at - moved);
            Self::access(cx, Way::Store, size, via, to, at - moved);
        }
        if moved != 0 {
            Self::offset(cx, from, from, -moved);
            Self::offset(cx, to, to, -moved);
        }
    }

    /// `dst = base + disp`.
    fn offset(cx: &mut Cx<'_, Self>, dst: Self::Reg, base: Self::Reg, disp: i64);
}
