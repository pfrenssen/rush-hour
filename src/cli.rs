//! Command-line flags, parsed with `std::env::args`.

use std::path::PathBuf;

pub const USAGE: &str = "\
Usage: rush-hour [OPTIONS]

Options:
  --level N            Start directly in level N (1-based)
  --screenshot PATH    Run a level for --frames frames, save a PNG screenshot and exit
  --frames F           Frames to run before the screenshot (default 600)
  -h, --help           Show this help
";

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Windowed,
    Screenshot { path: PathBuf, frames: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub mode: Mode,
    /// 0-based level index, if one was requested.
    pub level: Option<usize>,
}

#[derive(Debug, PartialEq)]
pub enum ParseOutcome {
    Run(Args),
    Help,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<ParseOutcome, String> {
    let mut level = None;
    let mut screenshot = None;
    let mut frames = 600;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            "--level" => {
                let v = value("--level")?;
                let n: usize = v
                    .parse()
                    .map_err(|_| format!("--level expects a number, got {v:?}"))?;
                if n == 0 || n > crate::levels::LEVELS.len() {
                    return Err(format!(
                        "--level must be between 1 and {}",
                        crate::levels::LEVELS.len()
                    ));
                }
                level = Some(n - 1);
            }
            "--screenshot" => screenshot = Some(PathBuf::from(value("--screenshot")?)),
            "--frames" => {
                let v = value("--frames")?;
                frames = v
                    .parse()
                    .map_err(|_| format!("--frames expects a number, got {v:?}"))?;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let mode = match screenshot {
        Some(path) => Mode::Screenshot { path, frames },
        None => Mode::Windowed,
    };
    Ok(ParseOutcome::Run(Args { mode, level }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<ParseOutcome, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_windowed() {
        assert_eq!(
            run(&[]).unwrap(),
            ParseOutcome::Run(Args {
                mode: Mode::Windowed,
                level: None,
            })
        );
    }

    #[test]
    fn parses_screenshot_mode() {
        let ParseOutcome::Run(args) =
            run(&["--screenshot", "out.png", "--level", "2", "--frames", "30"]).unwrap()
        else {
            panic!("expected run");
        };
        assert_eq!(args.level, Some(1));
        assert_eq!(
            args.mode,
            Mode::Screenshot {
                path: "out.png".into(),
                frames: 30
            }
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(run(&["--level", "0"]).is_err());
        assert!(run(&["--level", "99"]).is_err());
        assert!(run(&["--frames", "abc"]).is_err());
        assert!(run(&["--frames"]).is_err());
        assert!(run(&["--bogus"]).is_err());
        assert_eq!(run(&["--help"]).unwrap(), ParseOutcome::Help);
    }
}
