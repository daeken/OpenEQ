//! The `S3D`/`PFS` archive container used by `.s3d` and `.eqg` files.
//!
//! Layout:
//!
//! ```text
//! 0x00 u32  directory offset
//! 0x04 u32  magic "PFS " (0x20534650)
//! dir   u32  entry count
//!       (u32 crc, u32 offset, u32 size) * count
//! ```
//!
//! One of those entries is the directory itself, tagged with the reserved CRC
//! [`DIR_CRC`]. Its payload is a zlib stream listing the file names.
//!
//! Two details matter and are easy to get wrong:
//!
//! 1. The entry table is stored in *arbitrary* order, but the directory lists
//!    names in *ascending offset* order. Names therefore have to be matched to
//!    entries after sorting by offset, not by table position.
//! 2. Each payload is a sequence of blocks, `(u32 compressed_len, u32
//!    inflated_len, bytes...)`, and `compressed_len` covers a complete zlib
//!    stream (header and trailer included). Blocks concatenate to form the
//!    file.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;

use crate::{Error, Result};

/// Magic at offset 4 of a well-formed archive: the ASCII bytes "PFS ".
pub const PFS_MAGIC: u32 = 0x2053_4650;
/// CRC that tags the directory entry rather than a real file.
pub const DIR_CRC: u32 = 0x6158_0AC9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    offset: u32,
    size: u32,
}

/// An opened `S3D`/`PFS` archive. File contents are decompressed on demand.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    data: Vec<u8>,
    files: HashMap<String, Entry>,
    /// File names in the order the directory lists them.
    names: Vec<String>,
}

impl Archive {
    /// Opens an archive from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = std::fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        Self::from_bytes(path, data)
    }

    /// Parses an archive from an in-memory buffer.
    pub fn from_bytes(path: PathBuf, data: Vec<u8>) -> Result<Self> {
        let (files, names) = parse_directory(&data)?;
        Ok(Self {
            path,
            data,
            files,
            names,
        })
    }

    /// The path this archive was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// File names contained in the archive, in directory order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether the archive contains `name` (case-insensitive).
    pub fn contains(&self, name: &str) -> bool {
        self.files.contains_key(&name.to_ascii_lowercase())
    }

    /// Decompresses and returns the named file.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let key = name.to_ascii_lowercase();
        let entry = self
            .files
            .get(&key)
            .copied()
            .ok_or_else(|| Error::NotFound(name.to_owned()))?;
        self.inflate(entry)
    }

    /// Like [`read`](Self::read) but returns `Ok(None)` when the name is absent.
    pub fn read_opt(&self, name: &str) -> Result<Option<Vec<u8>>> {
        if !self.contains(name) {
            return Ok(None);
        }
        Ok(Some(self.read(name)?))
    }

    /// Iterates over every file, decompressing as it goes.
    pub fn iter_files(&self) -> impl Iterator<Item = Result<(String, Vec<u8>)>> + '_ {
        self.names.iter().cloned().map(|name| {
            let data = self.read(&name)?;
            Ok((name, data))
        })
    }

    fn inflate(&self, entry: Entry) -> Result<Vec<u8>> {
        let total = entry.size as usize;
        let mut out = Vec::with_capacity(total);
        let mut cursor = entry.offset as usize;
        let end = self.data.len();

        while out.len() < total {
            let header_end = cursor + 8;
            if header_end > end {
                return Err(Error::Truncated {
                    offset: cursor,
                    needed: 8,
                    available: end.saturating_sub(cursor),
                });
            }
            let compressed_len = read_u32(&self.data, cursor) as usize;
            let inflated_len = read_u32(&self.data, cursor + 4) as usize;
            cursor = header_end;

            if cursor + compressed_len > end {
                return Err(Error::Truncated {
                    offset: cursor,
                    needed: compressed_len,
                    available: end.saturating_sub(cursor),
                });
            }

            let block = &self.data[cursor..cursor + compressed_len];
            let mut decoder = ZlibDecoder::new(Cursor::new(block));
            let mut scratch = vec![0u8; inflated_len];
            decoder.read_exact(&mut scratch)?;
            out.extend_from_slice(&scratch);
            cursor += compressed_len;
        }

        out.truncate(total);
        Ok(out)
    }
}

fn parse_directory(data: &[u8]) -> Result<(HashMap<String, Entry>, Vec<String>)> {
    if data.len() < 8 {
        return Err(Error::Format("archive smaller than its header".into()));
    }
    let dir_offset = read_u32(data, 0) as usize;
    let magic = read_u32(data, 4);
    if magic != PFS_MAGIC {
        return Err(Error::BadMagic {
            found: magic,
            expected: PFS_MAGIC,
        });
    }
    if dir_offset + 4 > data.len() {
        return Err(Error::Truncated {
            offset: dir_offset,
            needed: 4,
            available: data.len().saturating_sub(dir_offset),
        });
    }

    let count = read_u32(data, dir_offset) as usize;
    let table_start = dir_offset + 4;
    let table_end = table_start
        .checked_add(count * 12)
        .ok_or_else(|| Error::Format("entry table length overflow".into()))?;
    if table_end > data.len() {
        return Err(Error::Truncated {
            offset: table_start,
            needed: count * 12,
            available: data.len().saturating_sub(table_start),
        });
    }

    let mut dir_entry = None;
    let mut files: Vec<Entry> = Vec::with_capacity(count);
    for i in 0..count {
        let base = table_start + i * 12;
        let crc = read_u32(data, base);
        let offset = read_u32(data, base + 4);
        let size = read_u32(data, base + 8);
        if crc == DIR_CRC {
            dir_entry = Some(Entry { offset, size });
        } else {
            files.push(Entry { offset, size });
        }
    }

    let dir_entry =
        dir_entry.ok_or_else(|| Error::Format("archive has no directory entry".into()))?;
    let directory = inflate_raw(data, dir_entry)?;

    let listed = read_u32(&directory, 0) as usize;
    if listed != files.len() {
        return Err(Error::DirectoryMismatch {
            listed,
            table: files.len(),
        });
    }

    // Names are ordered by ascending payload offset, but the table is not.
    files.sort_by_key(|entry| entry.offset);

    let mut names = Vec::with_capacity(listed);
    let mut map = HashMap::with_capacity(listed);
    let mut cursor = 4;
    for entry in &files {
        if cursor + 4 > directory.len() {
            return Err(Error::Format("directory name table truncated".into()));
        }
        let len = read_u32(&directory, cursor) as usize;
        cursor += 4;
        if cursor + len > directory.len() {
            return Err(Error::Format("directory name truncated".into()));
        }
        let raw = &directory[cursor..cursor + len];
        cursor += len;
        let name: String = raw
            .iter()
            .copied()
            .take_while(|b| *b != 0)
            .map(|b| b as char)
            .collect();
        map.insert(name.to_ascii_lowercase(), *entry);
        names.push(name);
    }

    Ok((map, names))
}

/// Decompresses a block sequence directly out of a borrowed buffer.
fn inflate_raw(data: &[u8], entry: Entry) -> Result<Vec<u8>> {
    let total = entry.size as usize;
    let mut out = Vec::with_capacity(total);
    let mut cursor = entry.offset as usize;
    while out.len() < total {
        if cursor + 8 > data.len() {
            return Err(Error::Truncated {
                offset: cursor,
                needed: 8,
                available: data.len().saturating_sub(cursor),
            });
        }
        let compressed_len = read_u32(data, cursor) as usize;
        let inflated_len = read_u32(data, cursor + 4) as usize;
        cursor += 8;
        if cursor + compressed_len > data.len() {
            return Err(Error::Truncated {
                offset: cursor,
                needed: compressed_len,
                available: data.len().saturating_sub(cursor),
            });
        }
        let mut decoder = ZlibDecoder::new(Cursor::new(&data[cursor..cursor + compressed_len]));
        let mut scratch = vec![0u8; inflated_len];
        decoder.read_exact(&mut scratch)?;
        out.extend_from_slice(&scratch);
        cursor += compressed_len;
    }
    out.truncate(total);
    Ok(out)
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
