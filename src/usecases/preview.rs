//! `preview`: shows a picked text. A URL opens with the system opener; an
//! image opens in the image popup; a text file or a directory opens in the
//! pager popup. A missing path or a binary file gives a notification.

use std::collections::BTreeMap;
use std::path::Path;

use super::ports::{Files, Launcher, PaneHost, PortError};
use crate::domain::preview::{self, Preview};
use crate::domain::settings::PopupSize;

/// The popup pane declared in `herdr-plugin.toml` that draws an image.
pub const VIEWER_ENTRYPOINT: &str = "viewer";
/// The popup pane declared in `herdr-plugin.toml` that pages a text file or
/// lists a directory.
pub const PAGER_ENTRYPOINT: &str = "pager";
/// Environment variable carrying the path to the popup.
pub const PATH_ENV: &str = "HERDR_FINGERS_PATH";

/// What a preview goes through.
pub struct PreviewDeps<'a> {
    pub host: &'a dyn PaneHost,
    pub files: &'a dyn Files,
    pub launcher: &'a dyn Launcher,
}

/// Previews `text`, a path relative to `cwd` or a URL, and returns a line
/// for the log.
pub fn preview(
    text: &str,
    cwd: Option<&Path>,
    plugin_id: &str,
    popup: &PopupSize,
    deps: &PreviewDeps<'_>,
) -> Result<String, PortError> {
    let target = if preview::is_url(text) {
        Preview::Url(text.to_string())
    } else {
        let path = preview::resolve(text, cwd, deps.files.home().as_deref());
        let entry = deps.files.entry(&path);
        preview::classify(path, entry)
    };
    let (entrypoint, path) = match target {
        Preview::Url(url) => {
            return match deps.launcher.open(&url, cwd) {
                Ok(()) => Ok(format!("opened {url}")),
                Err(error) => {
                    deps.host.notify("Cannot open URL", &error.to_string())?;
                    Ok(format!("cannot open {url}: {error}"))
                }
            };
        }
        Preview::Missing(path) => {
            deps.host.notify("Not found", &path.display().to_string())?;
            return Ok(format!("not found: {}", path.display()));
        }
        Preview::Binary(path) => {
            deps.host
                .notify("No preview for a binary file", &path.display().to_string())?;
            return Ok(format!("binary: {}", path.display()));
        }
        Preview::Image(path) => (VIEWER_ENTRYPOINT, path),
        Preview::Text(path) | Preview::Directory(path) => (PAGER_ENTRYPOINT, path),
    };
    let env = BTreeMap::from([(PATH_ENV.to_string(), path.display().to_string())]);
    deps.host.open_popup(plugin_id, entrypoint, env, popup)?;
    Ok(format!("previewed {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preview::Entry;
    use crate::usecases::testing::{FakeFiles, FakeHost, RecordingLauncher};

    fn run(text: &str, files: &FakeFiles) -> (FakeHost, RecordingLauncher, String) {
        let host = FakeHost::showing("");
        let launcher = RecordingLauncher::default();
        let deps = PreviewDeps {
            host: &host,
            files,
            launcher: &launcher,
        };
        let message = preview(
            text,
            Some(Path::new("/work")),
            "id",
            &PopupSize::default(),
            &deps,
        )
        .unwrap();
        (host, launcher, message)
    }

    fn popup_of(host: &FakeHost) -> (String, String) {
        let popups = host.popups.borrow();
        let (_, entrypoint, env) = &popups[0];
        (entrypoint.clone(), env[PATH_ENV].clone())
    }

    #[test]
    fn an_image_opens_the_viewer_with_the_resolved_path() {
        let files = FakeFiles::with("/work/plots/a.png", Entry::File { binary: true });
        let (host, _, _) = run("plots/a.png", &files);
        assert_eq!(
            popup_of(&host),
            (VIEWER_ENTRYPOINT.into(), "/work/plots/a.png".into())
        );
    }

    #[test]
    fn a_text_file_and_a_directory_open_the_pager() {
        let files = FakeFiles::with("/home/u/notes.md", Entry::File { binary: false });
        let (host, _, _) = run("~/notes.md", &files);
        assert_eq!(
            popup_of(&host),
            (PAGER_ENTRYPOINT.into(), "/home/u/notes.md".into())
        );

        let files = FakeFiles::with("/etc", Entry::Directory);
        let (host, _, _) = run("/etc", &files);
        assert_eq!(popup_of(&host), (PAGER_ENTRYPOINT.into(), "/etc".into()));
    }

    #[test]
    fn a_url_goes_to_the_system_opener() {
        let (host, launcher, _) = run("https://herdr.dev", &FakeFiles::default());
        assert_eq!(launcher.opened.borrow()[0].0, "https://herdr.dev");
        assert!(host.popups.borrow().is_empty());
    }

    #[test]
    fn a_missing_path_or_a_binary_file_only_notifies() {
        let (host, _, message) = run("gone.txt", &FakeFiles::default());
        assert_eq!(host.notified.borrow()[0].0, "Not found");
        assert_eq!(message, "not found: /work/gone.txt");

        let files = FakeFiles::with("/work/a.out", Entry::File { binary: true });
        let (host, _, _) = run("a.out", &files);
        assert_eq!(host.notified.borrow()[0].0, "No preview for a binary file");
        assert!(host.popups.borrow().is_empty());
    }
}
