use std::{
    hash::{BuildHasher, RandomState},
    path::Path,
};

use crate::parse::{convert, convert_in_mem, get_stats_file};
pub use error::Error;
pub use parse::{RawTrackRow, RowType, StructuredRows, Target};

mod consumers;
mod error;
mod parse;

pub fn parse(
    file: &Path,
    structured: bool,
    target: Target,
    output_path: String,
) -> Result<(), Error> {
    let path = get_stats_file(file, output_path.clone())?;
    convert(&path, target, output_path, structured)?;

    Ok(())
}

pub fn parse_in_mem(file: &Path, structured: RowType<'_>) -> Result<(), Error> {
    let path = get_stats_file(file, format!("tmp_{}.db", RandomState::new().hash_one(0)))?;
    convert_in_mem(&path, structured)
}
