// SPDX-License-Identifier: MIT
//! Simple JSONL writer for Lonis blocks.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use lonis_schema::block::{Block, BlockKind};
use serde_json;

/// Writes a stream of `Block<BlockKind>` values, one JSON object per line.
pub struct BlockStreamWriter {
    inner: File,
}

impl BlockStreamWriter {
    /// Create a new writer that truncates the file at `path`.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let f = File::create(path)?;
        Ok(Self { inner: f })
    }

    /// Serialize a block as a JSON line.
    pub fn write(&mut self, block: &Block<BlockKind>) -> io::Result<()> {
        let line = serde_json::to_string(block).map_err(std::io::Error::other)?;
        self.inner.write_all(line.as_bytes())?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }
}
