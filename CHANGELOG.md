# Changelog

All notable changes to herdr-fingers are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versions
[Semantic Versioning](https://semver.org/). Entries describe what a user of
the plugin notices; the build pipeline and the docs live in the commit history.

## [Unreleased]

## [0.2.2] - 2026-10-08

### Changed
- The image popup steps with `n`/`N` instead of `j`/`k` and the arrows,
  and closes only with `q` or `Esc`.

## [0.2.1] - 2026-10-08

### Removed
- OpenEXR images: their decoder depends on an unmaintained crate.

## [0.2.0] - 2026-10-08

### Added
- A plain hint opens a menu: preview the match or copy it (`:menu:`, the
  new default `main_action`). A multi-selection offers only the copy.
- Preview opens a URL in the browser, an image in a popup drawn with Kitty
  graphics, a text file in a pager popup (`bat`, else `less`, else `more`)
  and a directory as `ls -la`. A binary file or a missing path gives a
  notification.
- The image popup steps through the images of the directory with `j`/`k`
  and copies the path with `y`.
- `herdr-fingers open <path-or-url>` shows the preview from other programs,
  such as a file manager.
- An `image` pattern for image files, also without a `/` in the name.
- `popup_width` and `popup_height` set the size of the preview popups.

### Changed
- Only the patterns for paths and URLs are on by default: `url`, `path`,
  `image`, `git-status` and `diff`.

## [0.1.1] - 2026-10-08

### Fixed
- Hints show on every pane of a split tab, not only on the top-left one.
  When Herdr sizes the overlay to the focused pane, the pane is drawn at
  the overlay's top-left corner instead of off screen.

## [0.1.0] - 2026-09-22

### Added
- Hints over everything worth copying in the focused Herdr pane: paths,
  URLs, Git SHAs, IPv4 addresses, UUIDs, hex numbers, long numbers,
  Kubernetes resources and pod names, `git status` paths and branch names,
  diff headers — the tmux-fingers built-ins, ported one for one.
- Type a hint to copy its text through Herdr to the terminal you are
  looking at (OSC 52), so it works over SSH; hold Shift to type it into the
  pane instead, Ctrl to open it with the system opener, Alt for a custom
  command. Every action is configurable, including arbitrary shell commands
  fed on stdin with `MODIFIER` and `HINT` in their environment.
- Multi-select with Tab: pick several hints, confirm with Tab or Enter, get
  them joined by a configurable separator.
- The overlay redraws the pane where it is, with its own colors, even when
  the tab is split; a URL or path wrapped over two rows is one hint.
- Identical texts share one hint, and the shortest hints go to the bottom
  of the screen, where the freshest output is.
- Keyboard layouts from tmux-fingers (qwerty, azerty, qwertz, dvorak,
  colemak and their home-row and one-hand variants), or your own alphabet.
- A commented `config.toml` written on first run; custom patterns in Rust
  regex syntax with an optional `match` group.
- `herdr-fingers scan` to preview the hints a screen dump would get.
