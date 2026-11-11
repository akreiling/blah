//! Isle: a solo island adventure in the manner of *Island of Kesmai*, in one window.
//!
//! Usage: `isle [--new] [--save PATH] [--round SECONDS] [--seed N]`.
//!
//! A bare launch resumes the saved character, or starts a new one. `--new` starts over and keeps the old
//! save beside the new one. Rounds tick every `--round` seconds, ten by default, as the original's did.

mod save;
mod view;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, actions,
    prelude::*, size,
};

use isle_core::Game;

use crate::view::IsleView;

const USAGE: &str = "usage: isle [--new] [--save PATH] [--round SECONDS] [--seed N]";
/// The original's pace: one command every ten seconds.
const DEFAULT_ROUND: f32 = 10.0;

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
    round: f32,
    seed: Option<u64>,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args { fresh: false, save: None, round: DEFAULT_ROUND, seed: None };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--new" => parsed.fresh = true,
            "--save" => parsed.save = Some(PathBuf::from(args.next().ok_or("--save needs a path")?)),
            "--round" => {
                let n = args.next().ok_or("--round needs a number of seconds")?;
                parsed.round = n.parse().map_err(|_| format!("--round takes seconds, not {n:?}"))?;
                if !(0.5..=60.0).contains(&parsed.round) {
                    return Err("--round must be between 0.5 and 60 seconds".into());
                }
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
    let (game, path) = start(&args).unwrap_or_else(|e| {
        eprintln!("isle: {e}");
        std::process::exit(2);
    });
    eprintln!("Save: {}", path.display());
    let round = Duration::from_secs_f32(args.round);

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
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| IsleView::new(game, path, round, window, cx)))
            .expect("failed to open the Isle window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn arguments_default_to_the_original_pace() {
        let a = args(&[]).unwrap();
        assert_eq!((a.fresh, a.round, a.seed), (false, DEFAULT_ROUND, None));
        let a = args(&["--new", "--round", "3", "--seed", "9", "--save", "/tmp/x.json"]).unwrap();
        assert_eq!((a.fresh, a.round, a.seed), (true, 3.0, Some(9)));
        assert!(args(&["--round", "0"]).is_err());
        assert!(args(&["--seed"]).is_err());
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
        game.answer("fighter");
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
