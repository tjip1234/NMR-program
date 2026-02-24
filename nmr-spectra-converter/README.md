# NMR Spectra Converter

Convert NMR vendor data to NMRPipe format. A Rust reimplementation of the
`delta2pipe` and `bruk2pipe` utilities from the NMRPipe suite.

## Crates

| Crate | Type | Description |
|-------|------|-------------|
| **nmrpipe-core** | lib | FDATA header, enums, and parameter access for the NMRPipe format |
| **nmrpipe-io** | lib | Binary I/O, byte-swapping, type conversion, and digital-filter correction |
| **delta2pipe** | lib | JEOL Delta (.jdf) → NMRPipe converter |
| **bruk2pipe** | lib | Bruker SER/FID → NMRPipe converter |

The workspace also produces two standalone binaries: `delta2pipe` and `bruk2pipe`.

## Building

```sh
cargo build --release
```

Binaries are placed in `target/release/delta2pipe` and `target/release/bruk2pipe`.

## Usage

### delta2pipe

Convert a JEOL Delta file to NMRPipe format:

```sh
delta2pipe --in spectrum.jdf --out spectrum.dat
```

With digital-filter correction:

```sh
delta2pipe --in spectrum.jdf --df --out spectrum.dat
```

Options:

| Flag | Description |
|------|-------------|
| `--in <FILE>` | Input JEOL Delta `.jdf` file |
| `--out <FILE>` | Output NMRPipe file (`-` for stdout, default) |
| `--df` | Apply digital-filter (group-delay) correction |
| `--df-val <VAL>` | Override the DF correction value (implies `--df`) |
| `--tr <VAL>` | Override transition ratio |
| `--real-only` | Output only the real channel |
| `-v, --verb` | Verbose output |

### bruk2pipe

Convert a Bruker FID/SER file to NMRPipe format:

```sh
bruk2pipe --in fid --out spectrum.dat --swap -t dmx --decim 24 --dspfvs 10
```

Options:

| Flag | Description |
|------|-------------|
| `--in <FILE>` | Input Bruker SER/FID file |
| `--out <FILE>` | Output NMRPipe file (`-` for stdout, default) |
| `-t, --bruk-type <TYPE>` | Bruker type: `amx`, `dmx`, or `am` (default: `amx`) |
| `--swap` / `--noswap` | Byte-swap control |
| `--ws <N>` | Input word size in bytes (default: 4) |
| `--decim <N>` | DECIM value for DMX correction |
| `--dspfvs <N>` | DSPFVS value for DMX correction |
| `--grpdly <VAL>` | Group delay value for DMX correction |
| `--bo <N>` | Byte offset to skip at start |
| `--bad <VAL>` | Bad-point clipping threshold |
| `--ext` | Extract valid points only |
| `-v, --verb` | Verbose output |

## Using as a library

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
nmrpipe-core = { path = "path/to/nmr-spectra-converter/crates/nmrpipe-core" }
nmrpipe-io   = { path = "path/to/nmr-spectra-converter/crates/nmrpipe-io" }
delta2pipe   = { path = "path/to/nmr-spectra-converter/crates/delta2pipe" }
bruk2pipe    = { path = "path/to/nmr-spectra-converter/crates/bruk2pipe" }
```

### Example: convert a JEOL file in Rust

```rust
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = delta2pipe::DeltaOptions {
        apply_df: true,
        ..Default::default()
    };

    let mut input = BufReader::new(File::open("spectrum.jdf")?);
    let result = delta2pipe::delta_to_pipe(&mut input, &opts)?;

    // result.fdata  — the NMRPipe FDATA header (512 × f32)
    // result.planes — Vec<Vec<f32>> of output data planes

    println!("Dimensions: {}", result.fdata.dim_count());
    println!("Data points: {}", result.planes[0].len());
    Ok(())
}
```

## Project structure

```
nmr-spectra-converter/
├── Cargo.toml              # Workspace root + binary targets
├── README.md
├── src/
│   └── bin/
│       ├── delta2pipe.rs   # CLI binary for JEOL conversion
│       └── bruk2pipe.rs    # CLI binary for Bruker conversion
└── crates/
    ├── nmrpipe-core/       # FDATA header, constants, parameter access
    │   └── src/
    │       ├── lib.rs
    │       ├── fdata.rs    # 512-word FDATA header (from fdatap.h)
    │       ├── enums.rs    # QuadFlag, DimCode, etc.
    │       └── params.rs   # ND parameter system, dimension mapping
    ├── nmrpipe-io/         # I/O utilities
    │   └── src/
    │       ├── lib.rs
    │       ├── byteswap.rs # Byte-swap, int→float, word-size conversion
    │       ├── dfcorrect.rs# FFT-based digital-filter correction
    │       ├── reader.rs   # NMRPipe file reader
    │       └── writer.rs   # NMRPipe file writer
    ├── delta2pipe/         # JEOL Delta converter
    │   └── src/
    │       ├── lib.rs
    │       ├── convert.rs  # Main conversion pipeline
    │       ├── header.rs   # Delta binary header parser (1360 bytes)
    │       └── submatrix.rs# Submatrix → sequential data layout
    └── bruk2pipe/          # Bruker converter
        └── src/
            ├── lib.rs
            ├── convert.rs  # Main conversion pipeline
            ├── dmx.rs      # DMX digital-filter initialisation
            └── ser2fid.rs  # SER/FID byte-level conversion
```

## Testing

```sh
cargo test --all
```

## License

MIT
