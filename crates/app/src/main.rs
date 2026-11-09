//! The GPUI frontend: one window onto the world, with the wanderer at its center.
//!
//! Usage: `blah [seed]`. The seed is a `u64` and defaults to [`DEFAULT_SEED`].

mod view;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, actions,
    prelude::*, size,
};

use blah_core::Game;

use crate::view::GameView;

/// The world a bare `blah` opens.
const DEFAULT_SEED: u64 = 42;

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
    for key in ["q", "cmd-q"] {
        bindings.push(KeyBinding::new(key, Quit, None));
    }
    bindings
}

/// The seed from the command line: the first argument if there is one, else the default. Anything else is
/// an error rather than a guess.
fn seed_from_args() -> Result<u64, String> {
    let mut args = std::env::args().skip(1);
    let seed = match args.next() {
        None => DEFAULT_SEED,
        Some(arg) => {
            arg.parse().map_err(|_| format!("seed must be a whole number from 0 to {}, not {arg:?}", u64::MAX))?
        }
    };
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    Ok(seed)
}

fn main() {
    let seed = match seed_from_args() {
        Ok(seed) => seed,
        Err(e) => {
            eprintln!("blah: {e}\nusage: blah [seed]");
            std::process::exit(2);
        }
    };

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
            titlebar: Some(TitlebarOptions {
                title: Some(format!("blah · seed {seed}").into()), ..Default::default()
            }),
            is_resizable: false,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| GameView::new(Game::new(seed), window, cx)))
            .expect("failed to open the game window");
        cx.activate(true);
    });
}
