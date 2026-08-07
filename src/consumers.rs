use anyhow::Result;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufWriter, Write},
};

pub trait Consumer<'a, R>
where
    R: IntoEntry<Self>,
{
    fn consume(&mut self, iter: impl Iterator<Item = rusqlite::Result<R>>) -> Result<()>;
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

impl<'a, R> Consumer<'a, R> for CsvConsumer
where
    R: IntoEntry<Self, Entry = Vec<String>>,
{
    fn consume(&mut self, iter: impl Iterator<Item = rusqlite::Result<R>>) -> Result<()> {
        let mut c = 0;
        for item in iter {
            let item = item?;
            self.writer.write_record(item.into_entry())?;
            c += 1;
        }
        self.writer.flush()?;

        println!("Wrote {c} entries to file");

        Ok(())
    }
}

impl<'a, R> Consumer<'a, R> for JsonConsumer
where
    R: IntoEntry<Self> + Serialize,
{
    fn consume(&mut self, mut iter: impl Iterator<Item = rusqlite::Result<R>>) -> Result<()> {
        let mut c = 0;

        self.writer.write_all(b"[\n")?;

        let Some(first) = iter.next() else {
            return Ok(());
        };
        serde_json::to_writer(&mut self.writer, &first?)?;

        for item in iter {
            let item = item?;
            self.writer.write_all(b",\n")?;
            serde_json::to_writer(&mut self.writer, &item)?;
            c += 1;
        }

        self.writer.write_all(b"]")?;
        self.writer.flush()?;

        println!("Wrote {c} entries to file");

        Ok(())
    }
}
