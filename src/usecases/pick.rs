//! `pick`: read the pane, label the matches, let the user choose, act. The
//! `:menu:` action asks first whether to preview or copy the pick.

use std::path::Path;

use thiserror::Error;

use super::ports::{Clipboard, Files, Launcher, PaneHost, PickView, Picker, PortError};
use super::preview::{self as previewer, PreviewDeps};
use crate::domain::geometry::OverlayGeometry;
use crate::domain::matcher::find_candidates;
use crate::domain::preview::{MenuItem, menu_items};
use crate::domain::screen::Screen;
use crate::domain::session::{Outcome, Selection, Session};
use crate::domain::settings::{Action, Settings};

/// What one run of the overlay works on.
pub struct PickRequest<'a> {
    pub plugin_id: &'a str,
    pub pane_id: &'a str,
    /// Where the pane sits in the overlay; `None` when launched by hand.
    pub geometry: Option<&'a OverlayGeometry>,
    pub settings: &'a Settings,
    /// A one-line notice to show (for instance a configuration problem).
    pub notice: Option<&'a str>,
}

/// The adapters one run of the overlay goes through.
pub struct Deps<'a> {
    pub host: &'a dyn PaneHost,
    pub files: &'a dyn Files,
    pub clipboard: &'a mut dyn Clipboard,
    pub launcher: &'a dyn Launcher,
    pub picker: &'a mut dyn Picker,
}

#[derive(Debug, Error)]
pub enum PickError {
    #[error(transparent)]
    Port(#[from] PortError),
}

/// How the run ended, with a line for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    Cancelled,
    Done(String),
}

pub fn pick(request: &PickRequest<'_>, deps: &mut Deps<'_>) -> Result<Picked, PickError> {
    let width = match request.geometry {
        Some(geometry) => geometry.pane.width,
        None => deps
            .host
            .layout(request.pane_id)
            .ok()
            .and_then(|layout| {
                layout
                    .panes
                    .into_iter()
                    .find(|p| p.pane_id == request.pane_id)
            })
            .map_or(0, |p| p.rect.width),
    };
    let screen = Screen::from_ansi(&deps.host.read_visible(request.pane_id)?, width);
    let candidates = find_candidates(&screen, &request.settings.patterns);
    let mut session = Session::new(candidates, &request.settings.alphabet);
    let view = PickView {
        screen: &screen,
        theme: &request.settings.theme,
        geometry: request.geometry,
        notice: request.notice,
    };
    match deps.picker.pick(&view, &mut session)? {
        Outcome::Picked(selection) => {
            let cwd = deps.host.pane_cwd(request.pane_id).unwrap_or(None);
            let mut action = request
                .settings
                .actions
                .for_modifier(selection.modifier)
                .clone();
            if action == Action::Menu {
                let title = preview(&selection.texts.join(&request.settings.multi_separator));
                let items = menu_items(&selection.texts);
                action = match deps.picker.choose(&view, &session, &title, &items)? {
                    Some(MenuItem::Copy) => Action::Copy,
                    Some(MenuItem::Preview) => {
                        let preview_deps = PreviewDeps {
                            host: deps.host,
                            files: deps.files,
                            launcher: deps.launcher,
                        };
                        let message = previewer::preview(
                            &selection.texts[0],
                            cwd.as_deref(),
                            request.plugin_id,
                            &request.settings.popup,
                            &preview_deps,
                        )?;
                        return Ok(Picked::Done(message));
                    }
                    None => return Ok(Picked::Cancelled),
                };
            }
            let message = dispatch(&selection, &action, request, cwd.as_deref(), deps)?;
            Ok(Picked::Done(message))
        }
        Outcome::Cancelled | Outcome::Continue => Ok(Picked::Cancelled),
    }
}

/// Carries out `action` on the picked texts. `Menu` is resolved by the
/// caller before it gets here.
fn dispatch(
    selection: &Selection,
    action: &Action,
    request: &PickRequest<'_>,
    cwd: Option<&Path>,
    deps: &mut Deps<'_>,
) -> Result<String, PickError> {
    let settings = request.settings;
    let text = selection.texts.join(&settings.multi_separator);
    match action {
        Action::Copy => {
            deps.clipboard.copy(&text)?;
            if settings.notify_on_copy
                && let Err(error) = deps.host.notify("Copied", &preview(&text))
            {
                return Ok(format!("copied {} (toast failed: {error})", preview(&text)));
            }
            Ok(format!("copied {}", preview(&text)))
        }
        Action::Paste => {
            deps.host.send_text(request.pane_id, &text)?;
            Ok(format!("pasted {}", preview(&text)))
        }
        Action::Open => {
            for item in &selection.texts {
                deps.launcher.open(item, cwd)?;
            }
            Ok(format!("opened {}", preview(&text)))
        }
        Action::Shell(command_line) => {
            let env = [
                ("MODIFIER", selection.modifier.as_str().to_string()),
                ("HINT", selection.hint.clone()),
            ];
            deps.launcher.run(command_line, &text, &env, cwd)?;
            Ok(format!("ran {command_line}"))
        }
        Action::Menu | Action::Nothing => Ok(String::new()),
    }
}

/// The first line of `text`, shortened for a status line or a toast.
pub fn preview(text: &str) -> String {
    const MAX_CHARS: usize = 40;
    let first_line = text.lines().next().unwrap_or("");
    let mut shortened: String = first_line.chars().take(MAX_CHARS).collect();
    if first_line.chars().count() > MAX_CHARS || text.lines().count() > 1 {
        shortened.push('…');
    }
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::alphabet::Alphabet;
    use crate::domain::geometry::Rect;
    use crate::domain::preview::Entry;
    use crate::domain::session::{Key, Modifier};
    use crate::usecases::preview::{PAGER_ENTRYPOINT, PATH_ENV, VIEWER_ENTRYPOINT};
    use crate::usecases::testing::{
        FakeFiles, FakeHost, RecordingClipboard, RecordingLauncher, ScriptedPicker,
    };

    const SCREEN: &str = "edit /etc/hosts now\nsee https://x.io/docs\n";

    /// The defaults with a plain hint copying directly.
    fn settings() -> Settings {
        let mut settings = menu_settings();
        settings.actions.main = Action::Copy;
        settings
    }

    /// The defaults: a plain hint opens the menu.
    fn menu_settings() -> Settings {
        Settings {
            alphabet: Alphabet::custom("asdf").unwrap(),
            ..Settings::default()
        }
    }

    fn geometry() -> OverlayGeometry {
        OverlayGeometry {
            pane_id: "w1:p1".into(),
            area: Rect::new(0, 0, 80, 24),
            pane: Rect::new(0, 0, 80, 24),
        }
    }

    struct World {
        host: FakeHost,
        files: FakeFiles,
        clipboard: RecordingClipboard,
        launcher: RecordingLauncher,
        picker: ScriptedPicker,
    }

    impl World {
        fn new(keys: &[Key]) -> Self {
            World {
                host: FakeHost::showing(SCREEN),
                files: FakeFiles::default(),
                clipboard: RecordingClipboard::default(),
                launcher: RecordingLauncher::default(),
                picker: ScriptedPicker::new(keys),
            }
        }

        fn run(
            &mut self,
            settings: &Settings,
            geometry: Option<&OverlayGeometry>,
            notice: Option<&str>,
        ) -> Picked {
            let request = PickRequest {
                plugin_id: "id",
                pane_id: "w1:p1",
                geometry,
                settings,
                notice,
            };
            let mut deps = Deps {
                host: &self.host,
                files: &self.files,
                clipboard: &mut self.clipboard,
                launcher: &self.launcher,
                picker: &mut self.picker,
            };
            pick(&request, &mut deps).unwrap()
        }
    }

    // On SCREEN with alphabet "asdf": the bottom match (the URL) is "a",
    // the path above it is "s".

    #[test]
    fn typing_a_hint_copies_its_text_and_touches_nothing_else() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Main)]);
        let picked = world.run(&settings(), Some(&geometry()), None);
        assert_eq!(picked, Picked::Done("copied /etc/hosts".into()));
        assert_eq!(world.clipboard.copied, vec!["/etc/hosts"]);
        assert!(world.host.sent.borrow().is_empty());
        assert!(world.launcher.opened.borrow().is_empty());
        assert!(world.host.notified.borrow().is_empty());
    }

    #[test]
    fn shift_types_the_text_into_the_pane_the_hints_came_from() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Shift)]);
        world.run(&settings(), Some(&geometry()), None);
        assert_eq!(
            *world.host.sent.borrow(),
            vec![("w1:p1".to_string(), "/etc/hosts".to_string())]
        );
        assert!(world.clipboard.copied.is_empty());
    }

    #[test]
    fn ctrl_opens_the_match_from_the_pane_working_directory() {
        let mut world = World::new(&[Key::Hint('a', Modifier::Ctrl)]);
        world.host = world.host.in_directory("/work/repo");
        world.run(&settings(), Some(&geometry()), None);
        assert_eq!(
            *world.launcher.opened.borrow(),
            vec![("https://x.io/docs".to_string(), Some("/work/repo".into()))]
        );
    }

    #[test]
    fn alt_runs_the_shell_action_with_the_text_the_hint_and_the_modifier() {
        let mut settings = settings();
        settings.actions.alt = Action::Shell("xargs nvim".into());
        let mut world = World::new(&[Key::Hint('s', Modifier::Alt)]);
        let picked = world.run(&settings, Some(&geometry()), None);
        assert_eq!(picked, Picked::Done("ran xargs nvim".into()));
        let ran = world.launcher.ran.borrow();
        assert_eq!(ran[0].command_line, "xargs nvim");
        assert_eq!(ran[0].stdin, "/etc/hosts");
        assert_eq!(
            ran[0].env,
            vec![
                ("MODIFIER".into(), "alt".into()),
                ("HINT".into(), "s".into())
            ]
        );
    }

    #[test]
    fn cancelling_copies_nothing() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Main), Key::Escape]);
        world.picker = ScriptedPicker::new(&[Key::Escape]);
        assert_eq!(
            world.run(&settings(), Some(&geometry()), None),
            Picked::Cancelled
        );
        assert!(world.clipboard.copied.is_empty());
        assert!(world.host.sent.borrow().is_empty());
    }

    #[test]
    fn multi_select_joins_the_picks_with_the_separator() {
        let mut settings = settings();
        settings.multi_separator = "\n".into();
        let mut world = World::new(&[
            Key::Tab,
            Key::Hint('a', Modifier::Main),
            Key::Hint('s', Modifier::Main),
            Key::Enter,
        ]);
        world.run(&settings, Some(&geometry()), None);
        assert_eq!(
            world.clipboard.copied,
            vec!["https://x.io/docs\n/etc/hosts"]
        );
    }

    #[test]
    fn a_copy_toast_goes_through_the_host_when_enabled() {
        let mut settings = settings();
        settings.notify_on_copy = true;
        let mut world = World::new(&[Key::Hint('a', Modifier::Main)]);
        world.run(&settings, Some(&geometry()), None);
        assert_eq!(
            *world.host.notified.borrow(),
            vec![("Copied".to_string(), "https://x.io/docs".to_string())]
        );
    }

    #[test]
    fn the_notice_and_geometry_reach_the_picker() {
        let mut world = World::new(&[Key::Escape]);
        world.run(&settings(), Some(&geometry()), Some("config.toml ignored"));
        assert_eq!(world.picker.seen_notice, Some("config.toml ignored".into()));
        assert_eq!(world.picker.seen_geometry, Some(geometry()));
        assert!(world.host.layout_calls.borrow().is_empty());
    }

    #[test]
    fn without_geometry_the_pane_width_comes_from_the_host_layout() {
        let mut world = World::new(&[Key::Escape]);
        world.host = FakeHost::showing(SCREEN).with_layout(FakeHost::split_layout("w1:p1"));
        world.run(&settings(), None, None);
        assert_eq!(*world.host.layout_calls.borrow(), vec!["w1:p1".to_string()]);
    }

    #[test]
    fn a_host_that_cannot_read_the_pane_is_an_error() {
        let mut world = World::new(&[Key::Escape]);
        world.host = FakeHost::showing(SCREEN).failing_reads();
        let request = PickRequest {
            plugin_id: "id",
            pane_id: "w1:p1",
            geometry: None,
            settings: &settings(),
            notice: None,
        };
        let mut deps = Deps {
            host: &world.host,
            files: &world.files,
            clipboard: &mut world.clipboard,
            launcher: &world.launcher,
            picker: &mut world.picker,
        };
        assert!(matches!(pick(&request, &mut deps), Err(PickError::Port(_))));
    }

    #[test]
    fn the_menu_asks_about_the_pick_and_copy_copies() {
        let mut world = World::new(&[Key::Hint('s', Modifier::Main)]);
        world.picker =
            ScriptedPicker::new(&[Key::Hint('s', Modifier::Main)]).choosing(MenuItem::Copy);
        let picked = world.run(&menu_settings(), Some(&geometry()), None);
        assert_eq!(picked, Picked::Done("copied /etc/hosts".into()));
        assert_eq!(
            world.picker.seen_menu,
            Some((
                "/etc/hosts".to_string(),
                vec![MenuItem::Preview, MenuItem::Copy]
            ))
        );
        assert_eq!(world.clipboard.copied, vec!["/etc/hosts"]);
    }

    #[test]
    fn preview_from_the_menu_opens_the_popup_for_the_kind_of_file() {
        let mut world = World::new(&[]);
        world.picker =
            ScriptedPicker::new(&[Key::Hint('s', Modifier::Main)]).choosing(MenuItem::Preview);
        world.files = FakeFiles::with("/etc/hosts", Entry::File { binary: false });
        world.run(&menu_settings(), Some(&geometry()), None);
        let popups = world.host.popups.borrow();
        assert_eq!(popups[0].1, PAGER_ENTRYPOINT);
        assert_eq!(popups[0].2[PATH_ENV], "/etc/hosts");
        assert!(world.clipboard.copied.is_empty());
    }

    #[test]
    fn preview_of_an_image_relative_to_the_pane_opens_the_viewer() {
        let mut world = World::new(&[]);
        world.host = FakeHost::showing("wrote plot.png\n").in_directory("/work");
        world.picker =
            ScriptedPicker::new(&[Key::Hint('a', Modifier::Main)]).choosing(MenuItem::Preview);
        world.files = FakeFiles::with("/work/plot.png", Entry::File { binary: true });
        world.run(&menu_settings(), Some(&geometry()), None);
        let popups = world.host.popups.borrow();
        assert_eq!(popups[0].1, VIEWER_ENTRYPOINT);
        assert_eq!(popups[0].2[PATH_ENV], "/work/plot.png");
    }

    #[test]
    fn closing_the_menu_does_nothing() {
        let mut world = World::new(&[]);
        world.picker = ScriptedPicker::new(&[Key::Hint('s', Modifier::Main)]);
        assert_eq!(
            world.run(&menu_settings(), Some(&geometry()), None),
            Picked::Cancelled
        );
        assert!(world.clipboard.copied.is_empty());
        assert!(world.host.popups.borrow().is_empty());
    }

    #[test]
    fn a_multi_pick_menu_offers_only_copy() {
        let mut world = World::new(&[]);
        world.picker = ScriptedPicker::new(&[
            Key::Tab,
            Key::Hint('a', Modifier::Main),
            Key::Hint('s', Modifier::Main),
            Key::Enter,
        ])
        .choosing(MenuItem::Copy);
        world.run(&menu_settings(), Some(&geometry()), None);
        assert_eq!(world.picker.seen_menu.unwrap().1, vec![MenuItem::Copy]);
        assert_eq!(world.clipboard.copied, vec!["https://x.io/docs /etc/hosts"]);
    }

    #[test]
    fn previews_are_short_single_lines() {
        assert_eq!(preview("short"), "short");
        assert_eq!(preview("first\nsecond"), "first…");
        assert_eq!(preview(&"x".repeat(50)), format!("{}…", "x".repeat(40)));
    }
}
