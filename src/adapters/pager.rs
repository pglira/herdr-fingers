//! The text popup: pages a text file with `bat` (or `batcat`) when it is
//! installed, else with `less`, else with `more`; lists a directory with
//! `ls -la` in the same pager. `q` closes it.

use std::io;
use std::path::Path;
use std::process::Command;

/// Picks the tools at run time, so the popup works with whatever the host
/// has. `LESS` is reset so that options such as `-F` cannot close the popup
/// right away; `more` exits at the end of its input, so a prompt keeps the
/// output on screen.
const SCRIPT: &str = r#"
p="$1"
export LESS=-R
if command -v less >/dev/null 2>&1; then
    page() { less -R; }
else
    page() { more; printf '\n[press Enter to close]'; read -r _; }
fi
if [ -d "$p" ]; then
    if ls --color=always -d / >/dev/null 2>&1; then
        ls -la --color=always -- "$p" | page
    else
        ls -laG -- "$p" | page
    fi
    exit 0
fi
bat=$(command -v bat || command -v batcat || true)
if [ -n "$bat" ] && command -v less >/dev/null 2>&1; then
    exec "$bat" --paging=always --pager="less -R" --color=always --style=header,numbers -- "$p"
fi
page < "$p"
"#;

/// Shows `path` in the pager and returns when the user closes it.
pub fn show(path: &Path) -> io::Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(SCRIPT)
        .arg("sh")
        .arg(path)
        .status()?;
    Ok(())
}
