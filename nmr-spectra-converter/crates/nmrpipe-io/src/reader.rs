//! NMRPipe data reader: read header + spectral data from files or streams.

use nmrpipe_core::enums::HdrStatus;
use nmrpipe_core::fdata::*;
use std::io::{self, Read, Seek, SeekFrom};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Data truncated: expected {expected} bytes, got {got}")]
    Truncated { expected: usize, got: usize },
}

/// Read an NMRPipe FDATA header from a reader.
pub fn read_fdata_header<R: Read>(reader: &mut R) -> Result<(Fdata, HdrStatus), ReadError> {
    let mut buf = vec![0u8; FDATA_BYTES];
    reader.read_exact(&mut buf)?;
    Fdata::from_bytes(&buf).map_err(|e| ReadError::InvalidHeader(e.to_string()))
}

/// Read spectral data as f32 values from a reader.
///
/// `count`: number of f32 values to read.
/// `needs_swap`: if true, byte-swap each 4-byte word.
pub fn read_float_data<R: Read>(
    reader: &mut R,
    count: usize,
    needs_swap: bool,
) -> Result<Vec<f32>, ReadError> {
    let byte_count = count * 4;
    let mut buf = vec![0u8; byte_count];
    reader.read_exact(&mut buf)?;

    if needs_swap {
        super::byteswap::bswap4(&mut buf);
    }

    let data: Vec<f32> = buf
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    Ok(data)
}

/// Read a block of raw bytes from a reader.
pub fn read_raw_bytes<R: Read>(reader: &mut R, count: usize) -> Result<Vec<u8>, ReadError> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Skip N bytes in a seekable reader.
pub fn skip_bytes<R: Read + Seek>(reader: &mut R, count: i64) -> Result<(), ReadError> {
    reader.seek(SeekFrom::Current(count))?;
    Ok(())
}

/// Read a complete NMRPipe file: header + all spectral data.
pub fn read_nmrpipe_file<R: Read>(reader: &mut R) -> Result<(Fdata, Vec<f32>), ReadError> {
    let (fdata, status) = read_fdata_header(reader)?;
    let needs_swap = status == HdrStatus::Swapped;

    // Determine total data size
    let xsize = fdata.data[FDSIZE] as usize;
    let ysize = fdata.data[FDSPECNUM] as usize;

    let total = if ysize > 0 { xsize * ysize } else { xsize };

    let data = read_float_data(reader, total, needs_swap)?;
    Ok((fdata, data))
}
