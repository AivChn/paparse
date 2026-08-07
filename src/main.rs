use anyhow::{Context, Result, bail};
use clap::Parser;
use rusqlite::{Connection, Row};
use serde::Serialize;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};
use zip::{ZipArchive, result::ZipError};

use crate::consumers::{Consumer, CsvConsumer, IntoEntry, JsonConsumer};

mod consumers;

pub const TABLE_NAME: &str = "tracks";
pub const PATH_COLUMN: &str = "path";
pub const TRACK_NAME_COLUMN: &str = "readable_name";
pub const TRACK_LISTEN_COUNT_COLUMN: &str = "total_played_times";
pub const SQLITE_MAGIC_SEQUENCE: &[u8] = b"SQLite format 3";

enum DbFile {
    Tmp(PathBuf),
    Perm(PathBuf),
}

impl DbFile {
    fn get(&self) -> &Path {
        match self {
            Self::Tmp(p) | Self::Perm(p) => p,
        }
    }
}

impl Drop for DbFile {
    fn drop(&mut self) {
        if let Self::Tmp(path) = self {
            _ = fs::remove_file(path);
        }
    }
}

#[derive(Serialize)]
struct RawTrackRow {
    path: String,
    name: String,
    listen_count: u32,
}

impl IntoEntry<CsvConsumer> for RawTrackRow {
    type Entry = Vec<String>;

    fn into_entry(self) -> Self::Entry {
        vec![self.path, self.name, self.listen_count.to_string()]
    }
}

impl IntoEntry<JsonConsumer> for RawTrackRow {
    type Entry = Self;

    fn into_entry(self) -> Self::Entry {
        self
    }
}

#[derive(Serialize)]
struct StructuredRows {
    title: String,
    artist: Option<String>,
    listen_count: u32,
}

impl From<RawTrackRow> for StructuredRows {
    fn from(value: RawTrackRow) -> Self {
        let title = value.name;
        let listen_count = value.listen_count;
        let artist = PathBuf::from(value.path).file_prefix().and_then(|s| {
            s.to_str()
                .and_then(|s| s.strip_suffix(&format!("- {title}")))
                .map(str::to_string)
        });
        Self {
            title,
            artist,
            listen_count,
        }
    }
}

impl IntoEntry<CsvConsumer> for StructuredRows {
    type Entry = Vec<String>;

    fn into_entry(self) -> Self::Entry {
        vec![
            self.title,
            self.artist.unwrap_or("".to_string()),
            self.listen_count.to_string(),
        ]
    }
}

impl IntoEntry<JsonConsumer> for StructuredRows {
    type Entry = Self;

    fn into_entry(self) -> Self::Entry {
        self
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Target {
    Csv,
    Json,
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = match self {
            Self::Csv => "csv",
            Self::Json => "json",
        };
        write!(f, "{v}")
    }
}

#[derive(clap::Parser)]
struct Args {
    #[clap(long, default_value_t = Target::Csv, help="Output format, CSV and JSON supported")]
    target: Target,
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
        target,
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

    let path = get_stats_file(&file, out_name.clone())?;
    convert(&path, target, out_name, structured)?;

    Ok(())
}

fn convert(path: &DbFile, target: Target, mut out_name: String, structured: bool) -> Result<()> {
    let conn = Connection::open(path.get()).context("Could not open db file")?;

    if !conn.table_exists(None, TABLE_NAME)? {
        bail!("No tracks table found")
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT {PATH_COLUMN}, {TRACK_NAME_COLUMN}, {TRACK_LISTEN_COUNT_COLUMN} FROM {TABLE_NAME}"
    ))?;

    let tracks = stmt
        .query_map([], |row: &Row<'_>| {
            Ok(RawTrackRow {
                path: row.get(0)?,
                name: row.get(1)?,
                listen_count: row.get(2)?,
            })
        })
        .context("")?;

    match target {
        Target::Csv => {
            out_name.push_str(".csv");
            let writer = csv::Writer::from_writer(File::create(out_name)?);
            let mut consumer = CsvConsumer::new(writer);
            if structured {
                consumer.consume(tracks.map(|r| r.map(StructuredRows::from)))?;
            } else {
                consumer.consume(tracks)?;
            }
        }
        Target::Json => {
            out_name.push_str(".json");
            let writer = BufWriter::new(File::create(out_name)?);
            let mut consumer = JsonConsumer::new(writer);
            if structured {
                consumer.consume(tracks.map(|r| r.map(StructuredRows::from)))?;
            } else {
                consumer.consume(tracks)?;
            }
        }
    }

    Ok(())
}

fn verify_magic_sequence<F: Read>(f: &mut F) -> Result<()> {
    let mut buf = [0u8; SQLITE_MAGIC_SEQUENCE.len()];
    f.read_exact(&mut buf)?;

    if buf != SQLITE_MAGIC_SEQUENCE {
        bail!("File is not an sqlite db");
    }

    Ok(())
}

fn get_stats_file(path: &Path, mut out_name: String) -> Result<DbFile> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("poweramp-backup") => {
            let mut archive = File::open(path)
                .map_err(ZipError::from)
                .and_then(ZipArchive::new)
                .context("Failed to open archive file")?;

            let mut stats = archive
                .by_name("lists-export")
                .context("Failed to extract the db file")?;

            verify_magic_sequence(&mut stats)?;
            out_name.push_str("db");

            let mut f = File::create(&out_name).context("Failed to save file")?;
            f.write_all(SQLITE_MAGIC_SEQUENCE)?;
            std::io::copy(&mut stats, &mut f).context("Failed to copy data")?;
            Ok(DbFile::Tmp(PathBuf::from(out_name)))
        }
        None | Some("db") => {
            verify_magic_sequence(&mut File::open(path)?)?;
            Ok(DbFile::Perm(path.to_path_buf()))
        }

        Some(_) => bail!("Not a Poweramp backup file or a db file"),
    }
}
