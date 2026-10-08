//! What the menu offers for a picked text, and which viewer previews it: a
//! URL opens in the browser, a path opens by the kind of file it names.

use std::path::{Path, PathBuf};

/// Image file extensions the viewer decodes, lowercase.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "qoi", "tga", "pnm", "pbm",
    "pgm", "ppm", "hdr",
];

/// URL schemes the system opener handles.
const URL_PREFIXES: &[&str] = &["http://", "https://", "ftp://", "file://"];

/// One entry of the menu shown after a pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Preview,
    Copy,
}

impl MenuItem {
    /// The key that picks the entry.
    pub fn key(self) -> char {
        match self {
            MenuItem::Preview => 'p',
            MenuItem::Copy => 'c',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MenuItem::Preview => "Preview",
            MenuItem::Copy => "Copy to clipboard",
        }
    }
}

/// The menu for the picked `texts`: a preview shows one thing, so a
/// multi-selection only offers the copy.
pub fn menu_items(texts: &[String]) -> Vec<MenuItem> {
    if texts.len() == 1 {
        vec![MenuItem::Preview, MenuItem::Copy]
    } else {
        vec![MenuItem::Copy]
    }
}

/// What the file system holds at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Missing,
    Directory,
    /// A file; `binary` when its first bytes hold a NUL byte.
    File {
        binary: bool,
    },
}

/// How a picked text is previewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    Url(String),
    Image(PathBuf),
    Text(PathBuf),
    Directory(PathBuf),
    Missing(PathBuf),
    Binary(PathBuf),
}

pub fn is_url(text: &str) -> bool {
    URL_PREFIXES.iter().any(|prefix| text.starts_with(prefix))
}

/// Whether `path` has an image extension (case ignored).
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(ext)))
}

/// Turns the text on screen into a path: `~/` goes under `home`, a relative
/// path under the pane's working directory, an absolute path stays.
pub fn resolve(text: &str, cwd: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let Some(home) = home {
        if text == "~" {
            return home.to_path_buf();
        }
        if let Some(rest) = text.strip_prefix("~/") {
            return home.join(rest);
        }
    }
    let path = Path::new(text);
    match cwd {
        Some(cwd) if path.is_relative() => cwd.join(path),
        _ => path.to_path_buf(),
    }
}

/// The preview of `path`, given what the file system holds there. An image
/// extension wins over the binary check, so every image goes to the viewer.
pub fn classify(path: PathBuf, entry: Entry) -> Preview {
    match entry {
        Entry::Missing => Preview::Missing(path),
        Entry::Directory => Preview::Directory(path),
        Entry::File { .. } if is_image(&path) => Preview::Image(path),
        Entry::File { binary: true } => Preview::Binary(path),
        Entry::File { binary: false } => Preview::Text(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_pick_offers_preview_and_copy_a_multi_pick_only_copy() {
        assert_eq!(
            menu_items(&["a".into()]),
            vec![MenuItem::Preview, MenuItem::Copy]
        );
        assert_eq!(menu_items(&["a".into(), "b".into()]), vec![MenuItem::Copy]);
    }

    #[test]
    fn web_urls_are_urls_ssh_remotes_are_not() {
        assert!(is_url("https://herdr.dev"));
        assert!(is_url("file:///etc/hosts"));
        assert!(!is_url("git@github.com:a/b.git"));
        assert!(!is_url("/tmp/x"));
    }

    #[test]
    fn paths_resolve_against_home_and_the_pane_directory() {
        let home = Some(Path::new("/home/u"));
        let cwd = Some(Path::new("/work"));
        assert_eq!(
            resolve("~/a.txt", cwd, home),
            PathBuf::from("/home/u/a.txt")
        );
        assert_eq!(resolve("~", cwd, home), PathBuf::from("/home/u"));
        assert_eq!(
            resolve("src/a.rs", cwd, home),
            PathBuf::from("/work/src/a.rs")
        );
        assert_eq!(
            resolve("/etc/hosts", cwd, home),
            PathBuf::from("/etc/hosts")
        );
        assert_eq!(resolve("a.rs", None, None), PathBuf::from("a.rs"));
    }

    #[test]
    fn the_entry_and_the_extension_choose_the_preview() {
        let path = |p: &str| PathBuf::from(p);
        assert_eq!(
            classify(path("/a.PNG"), Entry::File { binary: true }),
            Preview::Image(path("/a.PNG"))
        );
        assert_eq!(
            classify(path("/a.rs"), Entry::File { binary: false }),
            Preview::Text(path("/a.rs"))
        );
        assert_eq!(
            classify(path("/a.bin"), Entry::File { binary: true }),
            Preview::Binary(path("/a.bin"))
        );
        assert_eq!(
            classify(path("/d"), Entry::Directory),
            Preview::Directory(path("/d"))
        );
        assert_eq!(
            classify(path("/x.png"), Entry::Missing),
            Preview::Missing(path("/x.png"))
        );
    }
}
