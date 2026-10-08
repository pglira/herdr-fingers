use std::process::ExitCode;

use herdr_fingers::adapters::config::DEFAULT_CONFIG;
use herdr_fingers::app;

const USAGE: &str = "\
herdr-fingers — tmux-fingers for Herdr

Usage:
  herdr-fingers start [--patterns a,b]   plugin action: open the hints overlay
  herdr-fingers ui                       the overlay itself (run by Herdr)
  herdr-fingers open <path-or-url>       preview a path or URL in a popup
  herdr-fingers view                     the image popup (run by Herdr)
  herdr-fingers page                     the text popup (run by Herdr)
  herdr-fingers scan [--width N] < dump  list the hints a screen dump would get
  herdr-fingers default-config           print the commented default config.toml
  herdr-fingers --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("start") => app::start(&args[1..]),
        Some("ui") => app::ui(),
        Some("open") => app::open(&args[1..]),
        Some("view") => app::view(),
        Some("page") => app::page(),
        Some("scan") => app::scan(&args[1..]),
        Some("default-config") => {
            print!("{DEFAULT_CONFIG}");
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("herdr-fingers {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("herdr-fingers: {error}");
            ExitCode::FAILURE
        }
    }
}
