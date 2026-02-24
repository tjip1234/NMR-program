//! NMRPipe core types: FDATA header, enums, and parameter access.
//!
//! This crate provides the foundational types for the NMRPipe data format,
//! ported from the C headers `fdatap.h`, `prec.h`, and `namelist.h`.

pub mod enums;
pub mod fdata;
pub mod params;

pub use enums::*;
pub use fdata::*;
pub use params::*;
