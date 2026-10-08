//! The real file system behind the [`Files`] port.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::domain::preview::Entry;
use crate::usecases::ports::Files;

/// How many leading bytes decide whether a file is binary.
const SNIFF_BYTES: u64 = 8192;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemFiles;

impl Files for SystemFiles {
    fn entry(&self, path: &Path) -> Entry {
        match std::fs::metadata(path) {
            Err(_) => Entry::Missing,
            Ok(meta) if meta.is_dir() => Entry::Directory,
            Ok(_) => Entry::File {
                binary: is_binary(path),
            },
        }
    }

    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// A file is binary when its first bytes hold a NUL byte; an unreadable
/// file counts as binary, as no pager can show it.
fn is_binary(path: &Path) -> bool {
    let mut head = Vec::new();
    match std::fs::File::open(path).and_then(|file| file.take(SNIFF_BYTES).read_to_end(&mut head)) {
        Ok(_) => head.contains(&0),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_told_apart_by_kind_and_content() {
        let dir = std::env::temp_dir().join(format!("herdr-fingers-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        std::fs::write(dir.join("a.bin"), [0x7f, b'E', 0, 1]).unwrap();
        let files = SystemFiles;
        assert_eq!(files.entry(&dir), Entry::Directory);
        assert_eq!(
            files.entry(&dir.join("a.txt")),
            Entry::File { binary: false }
        );
        assert_eq!(
            files.entry(&dir.join("a.bin")),
            Entry::File { binary: true }
        );
        assert_eq!(files.entry(&dir.join("gone")), Entry::Missing);
        let _ = std::fs::remove_dir_all(dir);
    }
}
