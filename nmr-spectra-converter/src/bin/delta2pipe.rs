//! delta2pipe — Convert JEOL Delta (.jdf) files to NMRPipe format.

use clap::Parser;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};

use nmrpipe_core::fdata::Fdata;

#[derive(Parser)]
#[command(
    name = "delta2pipe",
    version,
    about = "Convert JEOL Delta (.jdf) files to NMRPipe format"
)]
struct Cli {
    /// Input Delta file
    #[arg(short, long)]
    r#in: String,

    /// Output NMRPipe file (or - for stdout)
    #[arg(short, long, default_value = "-")]
    out: String,

    /// Convert only real data (no imaginary)
    #[arg(long, default_value_t = false)]
    real_only: bool,

    /// Apply digital filter correction
    #[arg(long, default_value_t = false)]
    df: bool,

    /// Digital filter correction value override (implies --df)
    #[arg(long)]
    df_val: Option<f32>,

    /// Transition ratio override
    #[arg(long)]
    tr: Option<f32>,

    /// Verbose mode
    #[arg(short, long, default_value_t = false)]
    verb: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let opts = delta2pipe::DeltaOptions {
        real_only: cli.real_only,
        apply_df: cli.df || cli.df_val.is_some(),
        df_val: cli.df_val,
        tr_val: cli.tr,
        verbose: cli.verb,
    };

    let mut input = BufReader::new(File::open(&cli.r#in)?);
    let result = delta2pipe::delta_to_pipe(&mut input, &opts)?;

    if cli.verb {
        eprintln!("Delta conversion complete.");
        eprintln!("  Dimensions: {}", result.fdata.dim_count());
        eprintln!("  Planes: {}", result.planes.len());
        if result.stored_df_val != 0.0 {
            eprintln!("  Digital filter (stored): {:.4}", result.stored_df_val);
        }
        if result.applied_df_val != 0.0 {
            eprintln!("  Digital filter (applied): {:.4}", result.applied_df_val);
        }
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
