use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("io error: {0}")]
    Stream(#[from] std::io::Error),
    #[error("invalid archive magic {found:#010x} (expected {expected:#010x})")]
    BadMagic { found: u32, expected: u32 },
    #[error("truncated data: needed {needed} bytes at offset {offset}, have {available}")]
    Truncated {
        offset: usize,
        needed: usize,
        available: usize,
    },
    #[error("archive directory mismatch: header lists {listed} files, table has {table}")]
    DirectoryMismatch { listed: usize, table: usize },
    #[error("file not found in archive: {0}")]
    NotFound(String),
    #[error("unsupported or corrupt image in {name}: {source}")]
    Image {
        name: String,
        #[source]
        source: image::ImageError,
    },
    #[error("unexpected format: {0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, Error>;
