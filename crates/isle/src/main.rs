//! Isle: a solo island adventure in the manner of *Island of Kesmai*, in one window.
//!
//! Usage: `isle [--new] [--save PATH] [--round SECONDS] [--seed N] [--control PATH]`.
//!
//! A bare launch resumes the saved character, or starts a new one. `--new` starts over and keeps the old
//! save beside the new one. Play is turn-based: each command is a round. `--round` puts rounds on a clock
//! instead, the way the original ticked every ten seconds.

mod control;
mod input;
mod save;
mod sprites;
mod view;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, actions,
    prelude::*, size,
};

use isle_core::Game;

use crate::view::IsleView;

const USAGE: &str = "usage: isle [--new] [--save PATH] [--round SECONDS] [--seed N] [--control PATH]";

actions!(
    isle,
    [
        /// Leave the game.
        Quit,
    ]
);

#[derive(Clone, Debug, PartialEq)]
struct Args {
    fresh: bool,
    save: Option<PathBuf>,
    /// Seconds per round, if rounds run on a clock.
    round: Option<f32>,
    seed: Option<u64>,
    control: Option<PathBuf>,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args { fresh: false, save: None, round: None, seed: None, control: None };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--new" => parsed.fresh = true,
            "--save" => parsed.save = Some(PathBuf::from(args.next().ok_or("--save needs a path")?)),
            "--control" => parsed.control = Some(PathBuf::from(args.next().ok_or("--control needs a path")?)),
            "--round" => {
                let n = args.next().ok_or("--round needs a number of seconds")?;
                let seconds: f32 = n.parse().map_err(|_| format!("--round takes seconds, not {n:?}"))?;
                if !(0.5..=60.0).contains(&seconds) {
                    return Err("--round must be between 0.5 and 60 seconds".into());
                }
                parsed.round = Some(seconds);
            }
            "--seed" => {
                let n = args.next().ok_or("--seed needs a number")?;
                parsed.seed = Some(n.parse().map_err(|_| format!("--seed takes a whole number, not {n:?}"))?);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(parsed)
}

fn start(args: &Args) -> Result<(Game, PathBuf), String> {
    let path = match &args.save {
        Some(path) => path.clone(),
        None => save::default_path()?,
    };
    let seed = args.seed.unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs())
    });
    if args.fresh {
        save::set_aside(&path)?;
        return Ok((Game::new(seed), path));
    }
    Ok((save::load(&path)?.unwrap_or_else(|| Game::new(seed)), path))
}

fn main() {
    let args = parse_args(std::env::args().skip(1)).unwrap_or_else(|e| {
        eprintln!("isle: {e}\n{USAGE}");
        std::process::exit(2);
    });
    let control = args.control.clone().map(control::Server::bind).transpose().unwrap_or_else(|e| {
        eprintln!("isle: control socket: {e}");
        std::process::exit(2);
    });
    let (game, path) = match start(&args) {
        Ok(started) => started,
        Err(e) => {
            drop(control);
            eprintln!("isle: {e}");
            std::process::exit(2);
        }
    };
    eprintln!("Save: {}", path.display());
    if let Some(path) = &args.control {
        eprintln!("Control: {}", path.display());
    }
    let round = args.round.map(Duration::from_secs_f32);
    // A controlled window must not take the keyboard from whoever launched it.
    let foreground = control.is_none();

    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![Menu { name: "Isle".into(), items: vec![MenuItem::action("Quit Isle", Quit)] }]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(view::WINDOW_WIDTH, view::WINDOW_HEIGHT), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Isle".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            is_resizable: false,
            focus: foreground,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let mut view = IsleView::new(game, path, round, window, cx);
                if let Some(server) = control {
                    view.control(server, cx);
                }
                view
            })
        })
        .expect("failed to open the Isle window");
        if foreground {
            cx.activate(true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn arguments_default_to_turns_and_take_an_optional_clock() {
        let a = args(&[]).unwrap();
        assert_eq!((a.fresh, a.round, a.seed), (false, None, None));
        let a = args(&["--new", "--round", "3", "--seed", "9", "--save", "/tmp/x.json"]).unwrap();
        assert_eq!((a.fresh, a.round, a.seed), (true, Some(3.0), Some(9)));
        assert_eq!(args(&["--round", "0.5"]).unwrap().round, Some(0.5));
        assert_eq!(args(&["--round", "60"]).unwrap().round, Some(60.0));
        for invalid in ["0", "0.49", "60.1", "NaN", "inf", "-inf"] {
            assert!(args(&["--round", invalid]).is_err());
        }
        assert!(args(&["--seed"]).is_err());
        assert!(a.control.is_none());
        assert_eq!(args(&["--control", "/tmp/isle.sock"]).unwrap().control, Some("/tmp/isle.sock".into()));
        assert!(args(&["--control"]).is_err());
        assert!(args(&["7"]).is_err());
    }

    #[test]
    fn new_sets_the_old_save_aside_and_a_bare_launch_resumes() {
        let dir = std::env::temp_dir().join(format!("isle-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("save.json");
        let launch = |list: &[&str]| {
            let mut a = args(list).unwrap();
            a.save = Some(path.clone());
            start(&a).unwrap()
        };
        let (mut game, _) = launch(&["--seed", "4"]);
        game.answer("Wren");
        save::write(&path, &game).unwrap();
        let (resumed, _) = launch(&[]);
        assert_eq!(resumed, game);
        let (fresh, _) = launch(&["--new", "--seed", "5"]);
        assert_eq!((fresh.seed(), fresh.round_number()), (5, 0));
        let kept = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(kept, 1, "the old save is set aside, not deleted");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
