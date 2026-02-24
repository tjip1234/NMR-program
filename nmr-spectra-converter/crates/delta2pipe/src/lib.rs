//! JEOL Delta → NMRPipe format converter.
//!
//! Ported from `delta2pipe.c`, `delta.h`, and `smxutil.c`.

pub mod header;
pub mod submatrix;
pub mod convert;

pub use convert::*;
