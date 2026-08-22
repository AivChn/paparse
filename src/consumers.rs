use serde::Serialize;
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use crate::error::Error;

pub trait Consumer<R>
where
    R: IntoEntry<Self>,
{
    fn consume(&mut self, iter: impl Iterator<Item = rusqlite::Result<R>>) -> Result<(), Error>;
}

pub trait IntoEntry<ConsumerType: ?Sized> {
    type Entry;
    fn into_entry(self) -> Self::Entry;
}

pub struct CsvConsumer {
    writer: csv::Writer<File>,
}

pub struct JsonConsumer {
    writer: BufWriter<File>,
}

impl CsvConsumer {
    pub fn new(writer: csv::Writer<File>) -> Self {
        Self { writer }
    }
}

impl JsonConsumer {
    pub fn new(writer: BufWriter<File>) -> Self {
        Self { writer }
    }
}

impl<R> Consumer<R> for CsvConsumer
where
    R: IntoEntry<Self, Entry = Vec<String>>,
{
    fn consume(&mut self, iter: impl Iterator<Item = rusqlite::Result<R>>) -> Result<(), Error> {
        let mut c = 0;
        for item in iter {
            let item = item?;
            self.writer
                .write_record(item.into_entry())
                .map_err(|_| Error::IoOutput)?;
            c += 1;
        }
        self.writer.flush().map_err(|_| Error::IoOutput)?;

        println!("Wrote {c} entries to file");

        Ok(())
    }
}

impl<R> Consumer<R> for JsonConsumer
where
    R: IntoEntry<Self> + Serialize,
{
    fn consume(
        &mut self,
        mut iter: impl Iterator<Item = rusqlite::Result<R>>,
    ) -> Result<(), Error> {
        let mut c = 0;

        self.writer.write_all(b"[\n").map_err(|_| Error::IoOutput)?;

        let Some(first) = iter.next() else {
            return Ok(());
        };
        serde_json::to_writer(&mut self.writer, &first?).map_err(|_| Error::IoOutput)?;

        for item in iter {
            let item = item?;
            self.writer.write_all(b",\n").map_err(|_| Error::IoOutput)?;
            serde_json::to_writer(&mut self.writer, &item).map_err(|_| Error::IoOutput)?;
            c += 1;
        }

        self.writer.write_all(b"]").map_err(|_| Error::IoOutput)?;
        self.writer.flush().map_err(|_| Error::IoOutput)?;

        println!("Wrote {c} entries to file");

        Ok(())
    }
}
