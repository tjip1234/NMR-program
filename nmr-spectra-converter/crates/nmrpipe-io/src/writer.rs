//! NMRPipe data writer: write header + spectral data to files or streams.

use nmrpipe_core::fdata::*;
use std::io::{self, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WriteError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Write an NMRPipe FDATA header to a writer.
pub fn write_fdata_header<W: Write>(writer: &mut W, fdata: &Fdata) -> Result<(), WriteError> {
    let buf = fdata.to_bytes();
    writer.write_all(&buf)?;
    Ok(())
}

/// Write spectral data as f32 values to a writer (native endian).
pub fn write_float_data<W: Write>(writer: &mut W, data: &[f32]) -> Result<(), WriteError> {
    // Write as native-endian bytes
    let mut buf = vec![0u8; data.len() * 4];
    for (i, &val) in data.iter().enumerate() {
        let bytes = val.to_ne_bytes();
        buf[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    writer.write_all(&buf)?;
    Ok(())
}

/// Write a complete NMRPipe file: header + spectral data.
pub fn write_nmrpipe_file<W: Write>(
    writer: &mut W,
    fdata: &Fdata,
    data: &[f32],
) -> Result<(), WriteError> {
    write_fdata_header(writer, fdata)?;
    write_float_data(writer, data)?;
    Ok(())
}

/// Write NMRPipe data as a pipe stream (header + vectors one at a time).
pub struct PipeWriter<W: Write> {
    writer: W,
    header_written: bool,
}

impl<W: Write> PipeWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            header_written: false,
        }
    }

    /// Write the header (must be called first).
    pub fn write_header(&mut self, fdata: &Fdata) -> Result<(), WriteError> {
        write_fdata_header(&mut self.writer, fdata)?;
        self.header_written = true;
        Ok(())
    }

    /// Write a single vector of spectral data (one row/column).
    pub fn write_vector(&mut self, data: &[f32]) -> Result<(), WriteError> {
        write_float_data(&mut self.writer, data)
    }

    /// Flush the writer.
    pub fn flush(&mut self) -> Result<(), WriteError> {
        self.writer.flush()?;
        Ok(())
    }

    /// Consume and return the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}
