//! The GPUI frontend: one window onto the world, with the wanderer at its center.
//!
//! Usage: `blah [seed] [--steps N]`. The seed is a `u64` and defaults to [`DEFAULT_SEED`].
//!
//! `--steps N` is a development aid, in every build: it plays N turns by a fixed rule before the window
//! opens (see [`autoplay`]), so a seed and a count always put the same scene on screen, monsters and a
//! fight included, for checking a change or taking a screenshot.

mod sprites;
mod view;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, actions,
    prelude::*, size,
};

use blah_core::{Dir, Game};

use crate::view::GameView;

/// The world a bare `blah` opens.
const DEFAULT_SEED: u64 = 42;
/// What to print when the command line can't be read.
const USAGE: &str = "usage: blah [seed] [--steps N]";

actions!(
    blah,
    [
        /// Step one tile up the screen.
        MoveNorth,
        /// Step one tile down the screen.
        MoveSouth,
        /// Step one tile right.
        MoveEast,
        /// Step one tile left.
        MoveWest,
        /// Stay put for a turn.
        Wait,
        /// Leave the game.
        Quit,
    ]
);

/// Every key the game listens to. Arrows, vi keys, and WASD all walk.
fn keymap() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for key in ["up", "k", "w"] {
        bindings.push(KeyBinding::new(key, MoveNorth, None));
    }
    for key in ["down", "j", "s"] {
        bindings.push(KeyBinding::new(key, MoveSouth, None));
    }
    for key in ["right", "l", "d"] {
        bindings.push(KeyBinding::new(key, MoveEast, None));
    }
    for key in ["left", "h", "a"] {
        bindings.push(KeyBinding::new(key, MoveWest, None));
    }
    for key in [".", "space"] {
        bindings.push(KeyBinding::new(key, Wait, None));
    }
    for key in ["q", "cmd-q"] {
        bindings.push(KeyBinding::new(key, Quit, None));
    }
    bindings
}

/// What the command line asked for.
struct Args {
    seed: u64,
    /// Turns to [`autoplay`] before the window opens, a development aid.
    steps: u32,
}

/// Reads `[seed] [--steps N]`. The seed defaults to [`DEFAULT_SEED`]. Anything else is an error rather
/// than a guess.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut seed = None;
    let mut steps = 0;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--steps" {
            let n = args.next().ok_or("--steps needs a number")?;
            steps = n.parse().map_err(|_| format!("--steps takes a whole number, not {n:?}"))?;
        } else if seed.is_none() {
            seed = Some(
                arg.parse().map_err(|_| format!("seed must be a whole number from 0 to {}, not {arg:?}", u64::MAX))?,
            );
        } else {
            return Err("too many arguments".into());
        }
    }
    Ok(Args { seed: seed.unwrap_or(DEFAULT_SEED), steps })
}

/// Plays `turns` turns by a fixed rule, so a seed and a count always give the same scene: a development
/// aid behind `--steps`. Each turn it heads for the nearest monster in view, longer axis first (attacking on
/// contact), and failing that tries east, south, north, and west in turn, waiting if every way is blocked.
/// Returns the log total before the last turn, so that turn's lines show as new.
fn autoplay(game: &mut Game, turns: u32) -> u64 {
    // With no turns played, everything logged so far is news.
    let mut mark = 0;
    for _ in 0..turns {
        mark = game.log().total();
        let (x, y) = game.position();
        let target = game
            .monsters_in_view()
            .map(|m| m.position)
            .min_by_key(|&(mx, my)| ((mx - x).abs() + (my - y).abs(), my, mx));
        let mut wanted: Vec<Dir> = match target {
            Some((mx, my)) => {
                let horizontal = if mx > x { Dir::East } else { Dir::West };
                let vertical = if my > y { Dir::South } else { Dir::North };
                let (dx, dy) = ((mx - x).abs(), (my - y).abs());
                let axes =
                    if dx >= dy { [(horizontal, dx), (vertical, dy)] } else { [(vertical, dy), (horizontal, dx)] };
                axes.into_iter().filter(|&(_, d)| d > 0).map(|(dir, _)| dir).collect()
            }
            None => Vec::new(),
        };
        wanted.extend([Dir::East, Dir::South, Dir::North, Dir::West]);
        match wanted.into_iter().find(|&d| game.can_step(d)) {
            Some(d) => _ = game.step(d),
            None => _ = game.wait(),
        }
    }
    mark
}

fn main() {
    let Args { seed, steps } = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("blah: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let mut game = Game::new(seed);
    let mark = autoplay(&mut game, steps);

    Application::new().run(move |cx: &mut App| {
        cx.bind_keys(keymap());
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![Menu { name: "blah".into(), items: vec![MenuItem::action("Quit blah", Quit)] }]);
        // One window is the whole game, so closing it ends the game.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(view::WINDOW_WIDTH, view::WINDOW_HEIGHT), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // The view draws its own dark title strip under the traffic lights.
            titlebar: Some(TitlebarOptions {
                title: Some(format!("blah · seed {seed}").into()),
                appears_transparent: true,
                ..Default::default()
            }),
            is_resizable: false,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| GameView::new(game, mark, window, cx)))
            .expect("failed to open the game window");
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
    fn arguments_are_a_seed_and_an_optional_step_count() {
        let a = args(&[]).unwrap();
        assert_eq!((a.seed, a.steps), (DEFAULT_SEED, 0));
        let a = args(&["7", "--steps", "46"]).unwrap();
        assert_eq!((a.seed, a.steps), (7, 46));
        let a = args(&["--steps", "3", "9"]).unwrap();
        assert_eq!((a.seed, a.steps), (9, 3));
        assert!(args(&["x"]).is_err());
        assert!(args(&["1", "2"]).is_err());
        assert!(args(&["--steps"]).is_err());
    }

    #[test]
    fn autoplay_is_deterministic_and_marks_the_last_turn() {
        let (mut a, mut b) = (Game::new(7), Game::new(7));
        assert_eq!(autoplay(&mut a, 46), autoplay(&mut b, 46));
        assert_eq!(a, b);
        assert_eq!(a.turn(), 46);
        assert_eq!(autoplay(&mut Game::new(7), 0), 0);
    }
}
