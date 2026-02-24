//! Bruker → NMRPipe conversion library.
//!
//! Converts Bruker serial (SER/FID) files to NMRPipe float format,
//! handling AMX (4-byte int), DMX (digital oversampling), and AM (3-byte int) data.

pub mod ser2fid;
pub mod dmx;
pub mod convert;

pub use convert::{bruker_to_pipe, BrukerOptions, BrukerResult, BrukerError, BrukerType};
