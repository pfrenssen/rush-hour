//! Command-line flags, parsed with `std::env::args`.

use std::path::PathBuf;

pub const USAGE: &str = "\
Usage: rush-hour [OPTIONS]

Options:
  --level N            Start directly in level N (1-based)
  --seed X             Seed for the traffic generator (default: random in the
                       windowed game, 1 for --headless and --screenshot)
  --autopilot          Let the built-in controller switch the lights
  --headless           Run the simulation without a window and print a report
  --seconds S          Simulated seconds for --headless (default 120)
  --screenshot PATH    Run for --frames frames, save a PNG screenshot and exit
                       (shows the main menu unless --level is given)
  --frames F           Frames to run before the screenshot (default 600)
  -h, --help           Show this help
";

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Windowed,
    Headless { seconds: f32 },
    Screenshot { path: PathBuf, frames: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub mode: Mode,
    /// 0-based level index, if one was requested.
    pub level: Option<usize>,
    pub seed: Option<u64>,
    pub autopilot: bool,
}

#[derive(Debug, PartialEq)]
pub enum ParseOutcome {
    Run(Args),
    Help,
}

fn number<T: std::str::FromStr>(name: &str, v: &str) -> Result<T, String> {
    v.parse()
        .map_err(|_| format!("{name} expects a number, got {v:?}"))
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<ParseOutcome, String> {
    let mut level = None;
    let mut seed = None;
    let mut autopilot = false;
    let mut headless = false;
    let mut seconds: f32 = 120.0;
    let mut screenshot = None;
    let mut frames = 600;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            "--level" => {
                let n: usize = number("--level", &value("--level")?)?;
                let count = crate::levels::LEVELS.len();
                if n == 0 || n > count {
                    return Err(format!("--level must be between 1 and {count}"));
                }
                level = Some(n - 1);
            }
            "--seed" => seed = Some(number("--seed", &value("--seed")?)?),
            "--autopilot" => autopilot = true,
            "--headless" => headless = true,
            "--seconds" => {
                seconds = number("--seconds", &value("--seconds")?)?;
                if !(seconds > 0.0 && seconds.is_finite()) {
                    return Err("--seconds must be positive".into());
                }
            }
            "--screenshot" => screenshot = Some(PathBuf::from(value("--screenshot")?)),
            "--frames" => frames = number("--frames", &value("--frames")?)?,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let mode = match (headless, screenshot) {
        (true, Some(_)) => return Err("--headless and --screenshot cannot be combined".into()),
        (true, None) => Mode::Headless { seconds },
        (false, Some(path)) => Mode::Screenshot { path, frames },
        (false, None) => Mode::Windowed,
    };
    Ok(ParseOutcome::Run(Args {
        mode,
        level,
        seed,
        autopilot,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<ParseOutcome, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    fn args(a: &[&str]) -> Args {
        match run(a).unwrap() {
            ParseOutcome::Run(args) => args,
            ParseOutcome::Help => panic!("unexpected help"),
        }
    }

    #[test]
    fn defaults_to_windowed() {
        assert_eq!(
            args(&[]),
            Args {
                mode: Mode::Windowed,
                level: None,
                seed: None,
                autopilot: false,
            }
        );
    }

    #[test]
    fn parses_headless_mode() {
        let a = args(&[
            "--headless",
            "--level",
            "3",
            "--seconds",
            "90",
            "--seed",
            "7",
            "--autopilot",
        ]);
        assert_eq!(a.mode, Mode::Headless { seconds: 90.0 });
        assert_eq!(a.level, Some(2));
        assert_eq!(a.seed, Some(7));
        assert!(a.autopilot);
    }

    #[test]
    fn parses_screenshot_mode() {
        let a = args(&["--screenshot", "out.png", "--level", "2", "--frames", "30"]);
        assert_eq!(a.level, Some(1));
        assert_eq!(
            a.mode,
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
        assert!(run(&["--seed", "abc"]).is_err());
        assert!(run(&["--seconds", "-1"]).is_err());
        assert!(run(&["--frames"]).is_err());
        assert!(run(&["--bogus"]).is_err());
        assert!(run(&["--headless", "--screenshot", "x.png"]).is_err());
        assert_eq!(run(&["--help"]).unwrap(), ParseOutcome::Help);
    }
}
