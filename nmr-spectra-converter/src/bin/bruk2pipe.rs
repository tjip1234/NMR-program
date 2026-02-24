//! bruk2pipe — Convert Bruker SER/FID files to NMRPipe format.

use clap::Parser;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};

use nmrpipe_core::fdata::Fdata;

#[derive(Parser)]
#[command(
    name = "bruk2pipe",
    version,
    about = "Convert Bruker SER/FID files to NMRPipe format"
)]
struct Cli {
    /// Input Bruker SER/FID file
    #[arg(short, long)]
    r#in: String,

    /// Output NMRPipe file (or - for stdout)
    #[arg(short, long, default_value = "-")]
    out: String,

    /// Bruker type: amx, dmx, or am
    #[arg(short = 't', long, default_value = "amx")]
    bruk_type: String,

    /// Swap bytes
    #[arg(long, default_value_t = false)]
    swap: bool,

    /// No byte swap
    #[arg(long, default_value_t = false)]
    noswap: bool,

    /// Int-to-float conversion
    #[arg(long, default_value_t = true)]
    i2f: bool,

    /// No int-to-float
    #[arg(long, default_value_t = false)]
    noi2f: bool,

    /// Word size (3, 4, or 8)
    #[arg(long, default_value_t = 4)]
    ws: usize,

    /// Byte offset
    #[arg(long, default_value_t = 0)]
    bo: usize,

    /// Bad point threshold
    #[arg(long, default_value_t = 8_000_000.0)]
    bad: f32,

    /// Extract valid points only
    #[arg(long, default_value_t = false)]
    ext: bool,

    /// DECIM value
    #[arg(long, default_value_t = 0)]
    decim: i32,

    /// DSPFVS value
    #[arg(long, default_value_t = 10)]
    dspfvs: i32,

    /// GRPDLY value
    #[arg(long, default_value_t = 0.0)]
    grpdly: f32,

    /// Verbose mode
    #[arg(short, long, default_value_t = false)]
    verb: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let bt = match cli.bruk_type.to_lowercase().as_str() {
        "dmx" => bruk2pipe::BrukerType::Dmx,
        "am" => bruk2pipe::BrukerType::Am,
        _ => bruk2pipe::BrukerType::Amx,
    };

    let swap_flag = if cli.noswap {
        false
    } else if cli.swap {
        true
    } else {
        cfg!(target_endian = "little")
    };

    let i2f_flag = if cli.noi2f { false } else { cli.i2f };

    let mut fdata = Fdata::new();
    fdata.init_default();
    fdata.fixfdata();

    // In a full deployment the caller would populate fdata from acqus/acqu2s.
    // For now we use defaults; pass dimension info via CLI flags or a wrapper script.

    let opts = bruk2pipe::BrukerOptions {
        bruk_type: bt,
        fdata,
        swap: swap_flag,
        i2f: i2f_flag,
        word_size: if bt == bruk2pipe::BrukerType::Am { 3 } else { cli.ws },
        byte_offset: cli.bo,
        bad_thresh: cli.bad,
        ext_flag: cli.ext,
        decim: cli.decim,
        dspfvs: cli.dspfvs,
        grpdly: cli.grpdly,
        aq_mod: bruk2pipe::convert::AQ_MOD_QSIM,
        fc: 1.0,
        skip_size: 4,
        zf_flag: true,
        verbose: cli.verb,
    };

    let mut input = BufReader::new(File::open(&cli.r#in)?);
    let result = bruk2pipe::bruker_to_pipe(&mut input, &opts)?;

    if cli.verb {
        eprintln!("Bruker conversion complete.");
        eprintln!("  Planes: {}", result.planes.len());
    }

    write_output(&cli.out, &result.fdata, &result.planes)?;
    Ok(())
}

fn write_output(
    out_path: &str,
    fdata: &Fdata,
    planes: &[Vec<f32>],
) -> Result<(), Box<dyn std::error::Error>> {
    let hdr_bytes = fdata.to_bytes();

    if out_path == "-" {
        let stdout = io::stdout();
        let mut out = BufWriter::new(stdout.lock());
        out.write_all(&hdr_bytes)?;
        for plane in planes {
            let data_bytes: Vec<u8> = plane.iter().flat_map(|f| f.to_ne_bytes()).collect();
            out.write_all(&data_bytes)?;
        }
        out.flush()?;
    } else {
        let mut out = BufWriter::new(File::create(out_path)?);
        out.write_all(&hdr_bytes)?;
        for plane in planes {
            let data_bytes: Vec<u8> = plane.iter().flat_map(|f| f.to_ne_bytes()).collect();
            out.write_all(&data_bytes)?;
        }
        out.flush()?;
    }

    Ok(())
}
