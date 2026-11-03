use rusqlite::{Connection, Row};
use std::{
    fmt::Display,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

use serde::{Deserialize, Serialize};

use crate::{
    consumers::{Consumer, CsvConsumer, IntoEntry, JsonConsumer},
    error::Error,
};

pub const TABLE_NAME: &str = "tracks";
pub const PATH_COLUMN: &str = "path";
pub const TRACK_NAME_COLUMN: &str = "readable_name";
pub const TRACK_LISTEN_COUNT_COLUMN: &str = "total_played_times";

pub const SQLITE_MAGIC_SEQUENCE: &[u8] = b"SQLite format 3";
const ZIP_MAGIC_SEQUENCE: &[u8] = b"PK\x03\x04";
const MAX_DB_SIZE: u64 = 10 * 1024 * 1024 * 1024;

pub enum DbFile {
    Tmp(PathBuf),
    Perm(PathBuf),
}

pub enum SourceType {
    Archive,
    Db,
}

#[derive(Deserialize, Debug, Clone, Copy, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Csv,
    Json,
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
pub struct RawTrackRow {
    pub path: String,
    pub name: String,
    pub listen_count: u32,
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

#[derive(Serialize, Clone)]
pub struct StructuredRows {
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub listen_count: u32,
}

impl From<RawTrackRow> for StructuredRows {
    fn from(value: RawTrackRow) -> Self {
        let title = value.name;
        let listen_count = value.listen_count;
        let artist = PathBuf::from(&value.path).file_prefix().and_then(|s| {
            s.to_str()
                .and_then(|s| s.strip_suffix(&format!(" - {title}")))
                .map(str::to_string)
        });
        Self {
            path: value.path,
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
            self.path,
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

impl From<&str> for Target {
    fn from(value: &str) -> Self {
        if value == "json" {
            Self::Json
        } else {
            Self::Csv
        }
    }
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

pub fn convert(
    path: &DbFile,
    target: Target,
    out_name: String,
    structured: bool,
) -> Result<(), Error> {
    let conn = Connection::open(path.get())?;

    if !conn.table_exists(None, TABLE_NAME)? {
        return Err(Error::TableNotFound);
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT {PATH_COLUMN}, {TRACK_NAME_COLUMN}, {TRACK_LISTEN_COUNT_COLUMN} FROM {TABLE_NAME}"
    ))?;

    let tracks = stmt.query_map([], |row: &Row<'_>| {
        Ok(RawTrackRow {
            path: row.get(0)?,
            name: row.get(1)?,
            listen_count: row.get::<_, Option<u32>>(2)?.unwrap_or(0),
        })
    })?;

    match target {
        Target::Csv => {
            let mut writer =
                csv::Writer::from_writer(File::create(out_name).map_err(|_| Error::IoOutput)?);
            if structured {
                writer
                    .write_record(["path", "artist", "title", "listen_count"])
                    .map_err(|_| Error::IoOutput)?;
                let mut consumer = CsvConsumer::new(writer);
                consumer.consume(tracks.map(|r| r.map(StructuredRows::from)))?;
            } else {
                writer
                    .write_record(["path", "name", "listen_count"])
                    .map_err(|_| Error::IoOutput)?;
                let mut consumer = CsvConsumer::new(writer);
                consumer.consume(tracks)?;
            }
        }
        Target::Json => {
            let writer = BufWriter::new(File::create(out_name).map_err(|_| Error::IoOutput)?);
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

pub enum RowType<'a> {
    Structured(&'a mut Vec<StructuredRows>),
    Unstructured(&'a mut Vec<RawTrackRow>),
}

pub fn convert_in_mem(path: &DbFile, structured: RowType<'_>) -> Result<(), Error> {
    let conn = Connection::open(path.get())?;

    if !conn.table_exists(None, TABLE_NAME)? {
        return Err(Error::TableNotFound);
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT {PATH_COLUMN}, {TRACK_NAME_COLUMN}, {TRACK_LISTEN_COUNT_COLUMN} FROM {TABLE_NAME}"
    ))?;

    let tracks = stmt.query_map([], |row: &Row<'_>| {
        Ok(RawTrackRow {
            path: row.get(0)?,
            name: row.get(1)?,
            listen_count: row.get::<_, Option<u32>>(2)?.unwrap_or(0),
        })
    })?;

    match structured {
        RowType::Structured(items) => {
            items.extend(
                tracks
                    .map(|s| s.map(StructuredRows::from))
                    .collect::<Result<Vec<StructuredRows>, rusqlite::Error>>()?,
            );
        }
        RowType::Unstructured(items) => {
            items.extend(tracks.collect::<Result<Vec<RawTrackRow>, rusqlite::Error>>()?);
        }
    };

    Ok(())
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

fn identify_file<F: Read>(f: &mut F) -> Option<SourceType> {
    let mut buf = [0u8; max(SQLITE_MAGIC_SEQUENCE.len(), ZIP_MAGIC_SEQUENCE.len())];

    f.read_exact(&mut buf).ok()?;

    if &buf[..SQLITE_MAGIC_SEQUENCE.len()] == SQLITE_MAGIC_SEQUENCE {
        Some(SourceType::Db)
    } else if &buf[..ZIP_MAGIC_SEQUENCE.len()] == ZIP_MAGIC_SEQUENCE {
        Some(SourceType::Archive)
    } else {
        None
    }
}

fn db_magic_bytes_present<F: Read>(f: &mut F) -> bool {
    let mut buf = [0u8; SQLITE_MAGIC_SEQUENCE.len()];
    f.read_exact(&mut buf)
        .is_ok_and(|_| buf == SQLITE_MAGIC_SEQUENCE)
}

pub fn get_stats_file(path: &Path, mut out_name: String) -> Result<DbFile, Error> {
    let mut f = File::open(path)?;
    match identify_file(&mut f) {
        Some(SourceType::Archive) => {
            let mut archive = ZipArchive::new(f)?;
            let mut stats = archive.by_name("lists-export")?;

            out_name.push_str("db");

            let mut f = File::create(&out_name).map_err(|_| Error::IoOutput)?;
            if !db_magic_bytes_present(&mut stats) {
                return Err(Error::NotADbFile);
            }

            f.write_all(SQLITE_MAGIC_SEQUENCE)?;
            let mut take = (&mut stats).take(MAX_DB_SIZE);
            let copied = std::io::copy(&mut take, &mut f).map_err(|_| Error::IoOutput)?;

            // DbFile's Drop impl is now responsible for the file
            drop(f);
            let result = Ok(DbFile::Tmp(PathBuf::from(out_name)));

            if copied == MAX_DB_SIZE && stats.read(&mut [0u8; 1]).map_err(|_| Error::IoOutput)? > 0
            {
                return Err(Error::ArchiveTooLarge);
            }

            result
        }
        Some(SourceType::Db) => Ok(DbFile::Perm(path.to_path_buf())),
        None => Err(Error::FileInvalid),
    }
}
