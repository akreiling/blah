//! The GPUI frontend: one window onto the world, with the wanderer at its center.
//!
//! Usage: `blah [seed] [--new] [--save PATH] [--steps N] [--combat-demo | --inferno-demo]`.
//! The seed is a `u64` and defaults to [`DEFAULT_SEED`].
//!
//! A normal launch resumes the newest saved character and autosaves after every turn; `--new` starts
//! another in its own file. `blah --list` prints the saved characters. The demos are never saved.
//!
//! `--steps N` is a development aid, in every build: it plays N turns by a fixed rule before the window
//! opens (see [`autoplay`]), so a seed and a count always put the same scene on screen, monsters and a
//! fight included, for checking a change or taking a screenshot.

mod commitment;
mod save;
mod sprites;
mod view;
mod workshop;

use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, actions,
    prelude::*, size,
};

use blah_core::{Dir, Game, Maneuver};

use crate::view::GameView;

/// The world a bare `blah` opens.
const DEFAULT_SEED: u64 = 42;
/// What to print when the command line can't be read.
const USAGE: &str =
    "usage: blah [seed] [--new] [--save PATH] [--steps N] [--combat-demo | --inferno-demo]\n       blah --list";

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
        /// Use the prepared technique north.
        TechniqueNorth,
        /// Use the prepared technique south.
        TechniqueSouth,
        /// Use the prepared technique east.
        TechniqueEast,
        /// Use the prepared technique west.
        TechniqueWest,
        /// Use the selected earned move north.
        EarnedMoveNorth,
        /// Use the selected earned move south.
        EarnedMoveSouth,
        /// Use the selected earned move east.
        EarnedMoveEast,
        /// Use the selected earned move west.
        EarnedMoveWest,
        /// Select the next earned-move slot.
        CycleEarnedMove,
        /// Stay put for a turn.
        Wait,
        /// Claim the cache underfoot.
        ClaimCache,
        /// Make an exposed camp to recover Health and Focus.
        Camp,
        /// Use a salve during a normal turn.
        UseSalve,
        /// Gather the herb patch underfoot.
        Gather,
        /// Drink a Focus draught during a normal turn.
        DrinkDraught,
        /// Use the landmark underfoot.
        UseLandmark,
        /// Cycle active relics safely at home.
        CycleRelics,
        /// Select the next unequipped cache weapon for trade.
        CycleTrade,
        /// Exchange the selected cache weapon for supplies at home.
        TradeWeapon,
        /// Open the composition workshop.
        OpenWorkshop,
        /// Open the adaptation commitment panel.
        OpenCommitment,
        /// Cancel the open panel.
        CancelWorkshop,
        /// Confirm a commitment or prepare the workshop draft.
        PrepareWorkshop,
        /// Cycle the draft weapon.
        WorkshopWeapon,
        /// Cycle the draft delivery.
        WorkshopDelivery,
        /// Cycle the draft aim.
        WorkshopAim,
        /// Toggle the draft payload.
        WorkshopPayload,
        /// Cycle the payload element.
        WorkshopElement,
        /// Cycle the payload trigger.
        WorkshopTrigger,
        /// Cycle the payload shape.
        WorkshopShape,
        /// Increase payload power.
        WorkshopPowerUp,
        /// Decrease payload power.
        WorkshopPowerDown,
        /// Load the dagger example into the draft.
        WorkshopDaggerExample,
        /// Load the staff example into the draft.
        WorkshopStaffExample,
        /// Toggle careful movement.
        ToggleSneak,
        /// Rest a turn at home to recover Health and Focus.
        RecoverFocus,
        /// Unleash the prepared ground payload.
        Unleash,
        /// Leave the game.
        Quit,
    ]
);

/// Every key the game listens to. Arrows, vi keys, and WASD all walk.
fn keymap() -> Vec<KeyBinding> {
    let mut bindings = Vec::new();
    for key in ["up", "k", "w"] {
        bindings.push(KeyBinding::new(key, MoveNorth, None));
        bindings.push(KeyBinding::new(&format!("shift-{key}"), TechniqueNorth, None));
        bindings.push(KeyBinding::new(&format!("alt-{key}"), EarnedMoveNorth, None));
    }
    for key in ["down", "j", "s"] {
        bindings.push(KeyBinding::new(key, MoveSouth, None));
        bindings.push(KeyBinding::new(&format!("shift-{key}"), TechniqueSouth, None));
        bindings.push(KeyBinding::new(&format!("alt-{key}"), EarnedMoveSouth, None));
    }
    for key in ["right", "l", "d"] {
        bindings.push(KeyBinding::new(key, MoveEast, None));
        bindings.push(KeyBinding::new(&format!("shift-{key}"), TechniqueEast, None));
        bindings.push(KeyBinding::new(&format!("alt-{key}"), EarnedMoveEast, None));
    }
    for key in ["left", "h", "a"] {
        bindings.push(KeyBinding::new(key, MoveWest, None));
        bindings.push(KeyBinding::new(&format!("shift-{key}"), TechniqueWest, None));
        bindings.push(KeyBinding::new(&format!("alt-{key}"), EarnedMoveWest, None));
    }
    for key in [".", "space"] {
        bindings.push(KeyBinding::new(key, Wait, None));
    }
    for key in ["q", "cmd-q"] {
        bindings.push(KeyBinding::new(key, Quit, None));
    }
    bindings.extend([
        KeyBinding::new("tab", CycleEarnedMove, None),
        KeyBinding::new("m", OpenWorkshop, None),
        KeyBinding::new("b", OpenCommitment, None),
        KeyBinding::new("escape", CancelWorkshop, None),
        KeyBinding::new("enter", PrepareWorkshop, None),
        KeyBinding::new("c", ToggleSneak, None),
        KeyBinding::new("r", RecoverFocus, None),
        KeyBinding::new("f", Unleash, None),
        KeyBinding::new("g", ClaimCache, None),
        KeyBinding::new("p", Camp, None),
        KeyBinding::new("v", UseSalve, None),
        KeyBinding::new("e", Gather, None),
        KeyBinding::new("t", DrinkDraught, None),
        KeyBinding::new("x", UseLandmark, None),
        KeyBinding::new("y", CycleRelics, None),
        KeyBinding::new("i", CycleTrade, None),
        KeyBinding::new("u", TradeWeapon, None),
        KeyBinding::new("1", WorkshopWeapon, None),
        KeyBinding::new("2", WorkshopDelivery, None),
        KeyBinding::new("3", WorkshopAim, None),
        KeyBinding::new("4", WorkshopPayload, None),
        KeyBinding::new("5", WorkshopElement, None),
        KeyBinding::new("6", WorkshopTrigger, None),
        KeyBinding::new("7", WorkshopShape, None),
        KeyBinding::new("shift-=", WorkshopPowerUp, None),
        KeyBinding::new("=", WorkshopPowerUp, None),
        KeyBinding::new("-", WorkshopPowerDown, None),
        KeyBinding::new("0", WorkshopDaggerExample, None),
        KeyBinding::new("9", WorkshopStaffExample, None),
    ]);
    bindings
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Demo {
    None,
    Combat,
    Inferno,
}

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Args {
    seed: u64,
    seed_explicit: bool,
    fresh: bool,
    save_path: Option<std::path::PathBuf>,
    /// Print the saved characters and exit.
    list: bool,
    /// Turns to [`autoplay`] before the window opens, a development aid.
    steps: u32,
    demo: Demo,
}

/// Reads `[seed] [--steps N] [--combat-demo | --inferno-demo]`. The seed defaults to [`DEFAULT_SEED`].
/// The named trials are mutually exclusive; anything else is an error rather than a guess.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut seed = None;
    let mut steps = 0;
    let mut demo = Demo::None;
    let mut fresh = false;
    let mut save_path = None;
    let mut list = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--list" {
            list = true;
        } else if arg == "--new" {
            fresh = true;
        } else if arg == "--save" {
            if save_path.is_some() {
                return Err("--save may be supplied only once".into());
            }
            save_path = Some(std::path::PathBuf::from(args.next().ok_or("--save needs a path")?));
        } else if arg == "--steps" {
            let n = args.next().ok_or("--steps needs a number")?;
            steps = n.parse().map_err(|_| format!("--steps takes a whole number, not {n:?}"))?;
        } else if arg == "--combat-demo" || arg == "--inferno-demo" {
            if demo != Demo::None {
                return Err("--combat-demo and --inferno-demo are mutually exclusive".into());
            }
            demo = if arg == "--combat-demo" { Demo::Combat } else { Demo::Inferno };
        } else if arg.starts_with('-') {
            return Err(format!("unknown option {arg:?}"));
        } else if seed.is_none() {
            seed = Some(
                arg.parse().map_err(|_| format!("seed must be a whole number from 0 to {}, not {arg:?}", u64::MAX))?,
            );
        } else {
            return Err("too many arguments".into());
        }
    }
    if list && (seed.is_some() || steps > 0 || demo != Demo::None || fresh || save_path.is_some()) {
        return Err("--list takes no other arguments".into());
    }
    if demo != Demo::None && (fresh || save_path.is_some()) {
        return Err("Developer demos are unsaved; do not combine them with --new or --save.".into());
    }
    Ok(Args { seed: seed.unwrap_or(DEFAULT_SEED), seed_explicit: seed.is_some(), steps, demo, fresh, save_path, list })
}

/// Plays `turns` turns by a fixed rule, so a seed and a count always give the same scene: a development
/// aid behind `--steps`. Each turn it heads for the nearest monster in view, longer axis first (attacking on
/// contact), and failing that tries east, south, north, and west in turn, waiting if every way is blocked.
/// Returns the log total before the last turn, so that turn's lines show as new.
#[cfg(test)]
fn autoplay(game: &mut Game, turns: u32) -> u64 {
    autoplay_saved(game, turns, &mut None).expect("unsaved autoplay cannot fail")
}

fn autoplay_saved(game: &mut Game, turns: u32, save: &mut Option<save::Save>) -> Result<u64, String> {
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
        let direction = wanted.into_iter().find(|&d| game.can_step(d));
        save::commit_action(game, save, |game| match direction {
            Some(d) => _ = game.step(d),
            None => _ = game.wait(),
        })?;
    }
    Ok(mark)
}

fn initial_game(seed: u64, demo: Demo) -> Result<Game, &'static str> {
    match demo {
        Demo::None => Ok(Game::new(seed)),
        Demo::Combat => Game::composition_trial(seed, Maneuver::backstab_burst()),
        Demo::Inferno => Game::inferno_trial(seed),
    }
}

/// The same startup path is used by the window and headless lifecycle tests.
fn start_game(args: &Args, directory: &std::path::Path) -> Result<(Game, Option<save::Save>, u64), String> {
    let (mut game, mut save) = if args.demo != Demo::None {
        (initial_game(args.seed, args.demo).map_err(str::to_owned)?, None)
    } else {
        let (save, game) = match &args.save_path {
            Some(path) if args.fresh => {
                let game = Game::new(args.seed);
                (save::Save::create(path, &game)?, game)
            }
            Some(path) => save::Save::load(path)?,
            None => save::automatic(directory, args.fresh, args.seed)?,
        };
        if args.seed_explicit && game.world().seed() != args.seed {
            return Err("The saved world's seed differs. Use --new to start another character.".into());
        }
        (game, Some(save))
    };
    // A resumed log is old news; only lines this launch adds show as new.
    let resumed = save.as_ref().is_some_and(|s| !s.created);
    let mark =
        if args.steps == 0 && resumed { game.log().total() } else { autoplay_saved(&mut game, args.steps, &mut save)? };
    Ok((game, save, mark))
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("blah: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let directory = if args.demo != Demo::None || args.save_path.is_some() {
        std::path::PathBuf::new()
    } else {
        save::directory().unwrap_or_else(|error| {
            eprintln!("blah: {error}");
            std::process::exit(2);
        })
    };
    if args.list {
        match save::list(&directory) {
            Ok(lines) if lines.is_empty() => println!("No saved characters in {}.", directory.display()),
            Ok(lines) => lines.iter().for_each(|line| println!("{line}")),
            Err(error) => {
                eprintln!("blah: {error}");
                std::process::exit(2);
            }
        }
        return;
    }
    let (game, save, mark) = start_game(&args, &directory).unwrap_or_else(|error| {
        eprintln!("blah: {error}");
        std::process::exit(2);
    });
    let seed = game.world().seed();
    if let Some(save) = &save {
        eprintln!("Save: {}", save.path.display());
    }

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
        cx.open_window(options, |window, cx| cx.new(|cx| GameView::new(game, save, mark, window, cx)))
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
    fn arguments_accept_seed_steps_and_one_optional_demo() {
        let a = args(&[]).unwrap();
        assert_eq!((a.seed, a.steps, a.demo), (DEFAULT_SEED, 0, Demo::None));
        let a = args(&["7", "--steps", "46"]).unwrap();
        assert_eq!((a.seed, a.steps, a.demo), (7, 46, Demo::None));
        let a = args(&["--steps", "3", "9"]).unwrap();
        assert_eq!((a.seed, a.steps, a.demo), (9, 3, Demo::None));
        let a = args(&["42", "--combat-demo"]).unwrap();
        assert_eq!(a.demo, Demo::Combat);
        let a = args(&["--inferno-demo", "--steps", "2"]).unwrap();
        assert_eq!((a.seed, a.steps, a.demo), (DEFAULT_SEED, 2, Demo::Inferno));
        assert!(args(&["x"]).is_err());
        assert!(args(&["1", "2"]).is_err());
        assert!(args(&["--steps"]).is_err());
        assert!(args(&["--combat-demo", "--inferno-demo"]).is_err());
        assert!(args(&["--combat-demo", "--combat-demo"]).is_err());
    }

    #[test]
    fn startup_resumes_the_latest_character_and_new_starts_another() {
        let dir = std::env::temp_dir().join(format!("blah-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (played, _, _) = start_game(&args(&["7", "--steps", "12"]).unwrap(), &dir).unwrap();
        let (resumed, _, _) = start_game(&args(&[]).unwrap(), &dir).unwrap();
        assert_eq!(resumed, played, "a bare launch resumes without replaying any turn");
        let (_, _, mark) = start_game(&args(&[]).unwrap(), &dir).unwrap();
        assert_eq!(mark, played.log().total(), "resumed lines are not shown as new");
        let (stepped, _, mark) = start_game(&args(&["--steps", "1"]).unwrap(), &dir).unwrap();
        assert_eq!(stepped.turn(), played.turn() + 1);
        assert_eq!(mark, played.log().total(), "the launch turn's lines show as new, older ones don't");
        assert!(start_game(&args(&["9"]).unwrap(), &dir).is_err(), "a different seed never loads silently");
        let (fresh, save, _) = start_game(&args(&["9", "--new"]).unwrap(), &dir).unwrap();
        assert_eq!((fresh.world().seed(), fresh.turn()), (9, 0));
        assert!(save.unwrap().path.ends_with("character-000002.sqlite3"));
        let (demo, save, _) = start_game(&args(&["--combat-demo"]).unwrap(), &dir).unwrap();
        assert!(save.is_none() && demo.maneuver() == &Maneuver::backstab_burst());
        assert!(args(&["--combat-demo", "--new"]).is_err());
        assert!(args(&["--list"]).unwrap().list);
        assert!(args(&["--list", "--new"]).is_err());
        let listed = save::list(&dir).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(
            listed[0].starts_with("   1  seed 7  turn 13") && listed[1].starts_with("   2  seed 9  turn 0"),
            "{listed:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn autoplay_is_deterministic_and_marks_the_last_turn() {
        let (mut a, mut b) = (Game::new(7), Game::new(7));
        assert_eq!(autoplay(&mut a, 46), autoplay(&mut b, 46));
        assert_eq!(a, b);
        assert_eq!(a.turn(), 46);
        assert_eq!(autoplay(&mut Game::new(7), 0), 0);
    }

    #[test]
    fn actual_cli_startup_selects_both_trials_and_keeps_default_play() {
        for (arguments, expected) in [
            (vec![], Maneuver::default()),
            (vec!["--combat-demo"], Maneuver::backstab_burst()),
            (vec!["--inferno-demo"], Maneuver::staff_inferno()),
        ] {
            let args = args(&arguments).unwrap();
            let game = initial_game(args.seed, args.demo).unwrap();
            assert_eq!(game.maneuver(), &expected);
            if args.demo == Demo::Inferno {
                assert_eq!(game.preview_area().unwrap().self_damage, 12);
            }
        }
        assert!(!keymap().is_empty(), "all configured keystrokes must parse");
    }

    #[test]
    fn earned_move_keybindings_cover_tab_and_alt_direction_aliases() {
        let bindings: Vec<_> = keymap().iter().map(|binding| format!("{binding:?}")).collect();
        for (action, directions) in [
            ("EarnedMoveNorth", ["up", "k", "w"]),
            ("EarnedMoveSouth", ["down", "j", "s"]),
            ("EarnedMoveEast", ["right", "l", "d"]),
            ("EarnedMoveWest", ["left", "h", "a"]),
        ] {
            for direction in directions {
                assert!(
                    bindings.iter().any(|binding| {
                        binding.contains(action)
                            && binding.contains("alt: true")
                            && binding.contains(&format!("key: \"{direction}\""))
                    }),
                    "missing Alt+{direction} binding for {action}: {bindings:#?}"
                );
            }
        }
        assert!(
            bindings.iter().any(|binding| binding.contains("CycleEarnedMove") && binding.contains("key: \"tab\"")),
            "missing Tab slot-cycle binding: {bindings:#?}"
        );
        assert!(
            bindings.iter().any(|binding| { binding.contains("OpenCommitment") && binding.contains("key: \"b\"") }),
            "missing B breakthrough binding: {bindings:#?}"
        );
    }

    #[test]
    fn gathering_keys_do_not_replace_east_movement() {
        let bindings: Vec<_> = keymap().iter().map(|binding| format!("{binding:?}")).collect();
        for (key, action) in [("e", "Gather"), ("t", "DrinkDraught"), ("d", "MoveEast")] {
            let matching: Vec<_> = bindings
                .iter()
                .filter(|b| {
                    b.contains(&format!("key: \"{key}\"")) && b.contains("shift: false") && b.contains("alt: false")
                })
                .collect();
            assert_eq!(matching.len(), 1, "{matching:?}");
            assert!(matching[0].contains(action), "{matching:?}");
        }
    }

    #[test]
    fn landmarks_and_relics_have_distinct_interaction_keys() {
        let bindings: Vec<_> = keymap().iter().map(|binding| format!("{binding:?}")).collect();
        for (key, action) in [("x", "UseLandmark"), ("y", "CycleRelics"), ("i", "CycleTrade"), ("u", "TradeWeapon")] {
            let matching: Vec<_> = bindings
                .iter()
                .filter(|binding| {
                    binding.contains(&format!("key: \"{key}\""))
                        && binding.contains("shift: false")
                        && binding.contains("alt: false")
                })
                .collect();
            assert_eq!(matching.len(), 1, "{matching:?}");
            assert!(matching[0].contains(action), "{matching:?}");
        }
    }
}
