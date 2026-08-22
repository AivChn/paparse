use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to read file: {0:?}")]
    FailedToReadFile(#[from] io::Error),
    #[error("Could not parse file, make sure its a db file or an archive")]
    FileInvalid,
    #[error("The found file is not a valid database")]
    NotADbFile,
    #[error("The stats table was not found in the database")]
    TableNotFound,
    #[error("Database file is invalid")]
    InvalidDb(#[from] rusqlite::Error),
    #[error("Archive is invalid")]
    InvalidArchive(#[from] zip::result::ZipError),
    #[error("This archive is too large")]
    ArchiveTooLarge,
    #[error("Failed to write to output file")]
    IoOutput,
}
