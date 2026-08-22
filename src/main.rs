use anyhow::Result;
use clap::Parser;
use std::{ffi::OsStr, path::PathBuf};

use crate::parse::{Target, convert, get_stats_file};

mod consumers;
mod error;
mod parse;

#[derive(clap::Parser)]
struct Args {
    #[clap(
        long,
        short,
        default_value_t = false,
        help = "Structured will tell the parser to try and parse the path as if it has the shape 'Artist - Title'"
    )]
    structured: bool,
    #[clap(short, long, help = "Location for the output")]
    output: Option<String>,
    file: PathBuf,
}

fn main() -> Result<()> {
    let Args {
        structured,
        output,
        file,
    } = Args::parse();

    let out_name = output.unwrap_or_else(|| {
        file.file_prefix()
            .unwrap_or(OsStr::new("tmp"))
            .to_os_string()
            .into_string()
            .expect("Failed to parse filename")
    });
    let target = Target::from(out_name.split(".").last().unwrap_or("csv"));

    let path = get_stats_file(&file, out_name.clone())?;
    convert(&path, target, out_name, structured)?;

    Ok(())
}
