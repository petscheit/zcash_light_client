use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Store;

#[derive(Serialize, Deserialize)]
struct Record {
    height: u32,
    header_hex: String,
}

pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let p = path.as_ref().to_path_buf();
        if let Some(dir) = p.parent()
            && !dir.exists()
        {
            create_dir_all(dir)?;
        }
        if !p.exists() {
            File::create(&p)?;
        }
        Ok(FileStore { path: p })
    }

    fn append_record(&self, rec: &Record) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(rec).map_err(|e| io::Error::other(e.to_string()))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    fn read_lines(&self) -> io::Result<impl Iterator<Item = io::Result<String>>> {
        let f = File::open(&self.path)?;
        Ok(BufReader::new(f).lines())
    }
}

impl Store for FileStore {
    fn put(&self, height: u32, header_hex: &str) -> io::Result<()> {
        self.append_record(&Record {
            height,
            header_hex: header_hex.to_string(),
        })
    }

    fn get(&self, height: u32) -> io::Result<Option<String>> {
        let mut found: Option<String> = None;
        for line in self.read_lines()? {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<Record>(&l)
                && rec.height == height
            {
                found = Some(rec.header_hex);
            }
        }
        Ok(found)
    }

    fn tip(&self) -> io::Result<Option<u32>> {
        let mut tip: Option<u32> = None;
        for line in self.read_lines()? {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<Record>(&l) {
                tip = Some(rec.height);
            }
        }
        Ok(tip)
    }

    fn last_n(&self, n: usize) -> io::Result<Vec<(u32, String)>> {
        let mut recs: Vec<(u32, String)> = Vec::new();
        for line in self.read_lines()? {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<Record>(&l) {
                recs.push((rec.height, rec.header_hex));
            }
        }
        if recs.len() > n {
            recs.drain(0..(recs.len() - n));
        }
        Ok(recs)
    }
}
