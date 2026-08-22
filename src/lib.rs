use std::path::Path;

use crate::parse::{convert, get_stats_file};
pub use error::Error;
pub use parse::Target;

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
