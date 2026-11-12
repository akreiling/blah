//! The window: the map on the left drawn as layered pixel tiles, the character panel on the right, the
//! log beneath, and the command line at the bottom. Commands resolve rounds, unless a clock is requested.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant as Clock};

use gpui::{
    App, Context, Div, FocusHandle, Focusable, FontWeight, KeyDownEvent, Keystroke, Pixels, SharedString, Subscription,
    Task, Window, div, img, prelude::*, px, rgb, rgba,
};

use isle_core::{
    Command, Creature, Game, Instant, Item, Kind, LandmarkKind, Parsed, Phase, Player, Pos, Practice, SIGHT, Tile,
    Tone, Zone, distance,
};

use crate::art::{ArtCache, GroundCell, Object, TerrainArt, item_art, terrain_art};
use crate::hero::Look;
use crate::input::{CommandLine, InputAction};
use crate::mood::{self, FRAME_MS, FRAMES};
use crate::sprites::SpriteCache;

/// Cells along each side of the map.
const VIEW: i32 = 2 * SIGHT + 1;
const CELL_W: f32 = 32.0;
const CELL_H: f32 = 32.0;
const GLYPH: f32 = 20.0;
const TITLE: f32 = 28.0;
const GAP: f32 = 12.0;
const BORDER: f32 = 2.0;
const INSET: f32 = 12.0;
const PANEL: f32 = 330.0;
const LINE: f32 = 20.0;
const LOG_LINES: usize = 10;
const PROMPT: f32 = 38.0;
/// Rounds the queue holds.
const QUEUE: usize = 5;
/// How often the clock is checked.
const TICK: Duration = Duration::from_millis(100);

const MAP_W: f32 = VIEW as f32 * CELL_W + 2.0 * BORDER;
const MAP_H: f32 = VIEW as f32 * CELL_H + 2.0 * BORDER;
const LOG_H: f32 = LOG_LINES as f32 * LINE + 2.0 * (INSET + BORDER);

/// Window width.
pub const WINDOW_WIDTH: Pixels = px(GAP + MAP_W + GAP + PANEL + GAP);
/// Window height.
pub const WINDOW_HEIGHT: Pixels = px(TITLE + MAP_H + GAP + LOG_H + GAP + PROMPT + GAP);

const FONT: &str = "Menlo";
const SCREEN: u32 = 0x07_08_0b;
const FRAME: u32 = 0x55_5a_88;
const TEXT: u32 = 0xd8_d8_c8;
const DIM: u32 = 0x76_79_88;
const FAINT: u32 = 0x45_47_52;
const HEADING: u32 = 0xf0_c0_4a;
const YOU: u32 = 0xff_f2_7a;
const HP: u32 = 0xd0_48_48;
const FOCUS: u32 = 0x5c_8c_f0;
const CLOCK: u32 = 0x8c_c0_6a;
const SET_GLOW: u32 = 0xb9_91_d5;
const EMPTY_BAR: u32 = 0x20_22_2a;
const CAVE_LIGHT: u32 = 0x0c_23_20;

/// The one window's view.
pub struct IsleView {
    game: Game,
    path: PathBuf,
    save_error: Option<String>,
    focus: FocusHandle,
    input: CommandLine,
    queue: VecDeque<(String, Command)>,
    /// The round clock; without one, each command is a round and the world waits.
    round: Option<Duration>,
    started: Clock,
    paused: bool,
    show_skills: bool,
    sprites: SpriteCache,
    art: ArtCache,
    /// Log total before the latest round: lines past it are news.
    mark: u64,
    _ticker: Option<Task<()>>,
    /// Animation time starts with the window; it never touches the game.
    born: Clock,
    /// Whether the last frame showed anything that moves, so idle maps cost nothing.
    animated: bool,
    forced_mood: mood::Override,
    _animator: Task<()>,
    control: Option<crate::control::Server>,
    _control: Option<(Task<()>, Subscription)>,
}

impl IsleView {
    /// A view of `game`, saved to `path`, turn-based or with rounds on a `round` clock.
    pub fn new(
        game: Game,
        path: PathBuf,
        round: Option<Duration>,
        window: &mut Window,
        cx: &mut Context<IsleView>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let ticker = round.map(|_| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    if this.update(cx, |view, cx| view.tick(cx)).is_err() {
                        break;
                    }
                }
            })
        });
        let animator = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(FRAME_MS)).await;
                let next = this.update(cx, |view, cx| {
                    if view.animated {
                        cx.notify();
                    }
                });
                if next.is_err() {
                    break;
                }
            }
        });
        let mark = game.log_total().saturating_sub(1);
        let sprites = SpriteCache::new(game.seed());
        let art = ArtCache::new(game.seed());
        IsleView {
            game,
            path,
            save_error: None,
            focus,
            input: CommandLine::default(),
            queue: VecDeque::new(),
            round,
            started: Clock::now(),
            paused: false,
            show_skills: false,
            sprites,
            art,
            mark,
            _ticker: ticker,
            born: Clock::now(),
            animated: false,
            forced_mood: mood::Override::default(),
            _animator: animator,
            control: None,
            _control: None,
        }
    }

    /// Preview a sky or stone regardless of the round or floor.
    pub fn force_mood(&mut self, forced: mood::Override) {
        self.forced_mood = forced;
    }

    fn save(&mut self) {
        self.save_error = crate::save::write(&self.path, &self.game).err();
    }

    fn tick(&mut self, cx: &mut Context<IsleView>) {
        let Some(round) = self.round else { return };
        if self.game.phase() != &Phase::Play || self.paused {
            self.started = Clock::now();
            return;
        }
        if self.started.elapsed() >= round {
            self.mark = self.game.log_total();
            let command = self.queue.pop_front().map(|(_, c)| c);
            self.game.round(command);
            self.save();
            self.started = Clock::now();
        }
        cx.notify();
    }

    fn key(&mut self, stroke: &Keystroke, cx: &mut Context<IsleView>) {
        if stroke.key == "escape" {
            self.show_skills = false;
        }
        match self.input.key(stroke, self.game.phase() == &Phase::Play) {
            InputAction::Submit => self.submit(cx),
            InputAction::Move(dir) => self.play_line(dir.short(), cx),
            InputAction::Command(command) => self.play_line(command, cx),
            InputAction::Edit => {}
            InputAction::Ignore => return,
        }
        cx.notify();
    }

    /// Process debug input on the UI thread, through exactly the same handler as the keyboard.
    pub fn control(&mut self, server: crate::control::Server, cx: &mut Context<Self>) {
        self.control = Some(server);
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(crate::control::POLL).await;
                loop {
                    let handled = this.update(cx, |view, cx| {
                        let Some(request) = view.control.as_ref().and_then(|s| s.requests.try_recv().ok()) else {
                            return false;
                        };
                        view.serve(request, cx);
                        true
                    });
                    match handled {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(_) => return,
                    }
                }
            }
        });
        // Quitting does not drop the view, so close the socket here.
        let quit = cx.on_app_quit(|view, _| {
            view.control = None;
            async {}
        });
        self._control = Some((task, quit));
    }

    fn serve(&mut self, request: crate::control::Request, cx: &mut Context<Self>) {
        let plan = &request.plan;
        let mark = self.game.log_total();
        let mut steps = 0;
        let mut interrupted = false;
        for key in &plan.keys {
            let (step_mark, before) = (self.game.log_total(), sighted_ids(&self.game));
            self.key(key, cx);
            steps += 1;
            if plan.interrupt && steps < plan.keys.len() && setback(&self.game, step_mark, &before) {
                interrupted = true;
                break;
            }
        }
        let _ = request.reply.send(self.report(&plan.view, mark, steps, interrupted));
    }
    /// What a control request did, and whatever else it asked to see.
    fn report(&self, view: &crate::control::View, mark: u64, steps: usize, interrupted: bool) -> serde_json::Value {
        use crate::control::Map;
        let game = &self.game;
        let news = ((game.log_total() - mark) as usize).min(game.log().len());
        let mut report = serde_json::json!({
            "steps": steps,
            "interrupted": interrupted,
            "round": game.round_number(),
            "news": &game.log()[game.log().len() - news..],
        });
        if view.state {
            report["screen"] = self.screen();
        }
        let level = game.level();
        let labels: Vec<(Pos, char)> = game.sighted().iter().map(|s| (s.creature.pos, s.label)).collect();
        // Keep town and Deep coordinates stable; in the wilderness return a bounded local window.
        let (ox, oy) = if let Some(town) = level.town_at(game.player().pos) {
            town.origin
        } else if level.deep().is_some() {
            (0, 0)
        } else {
            (game.player().pos.0 - level.width / 2, game.player().pos.1 - level.height / 2)
        };
        let rows = |f: &dyn Fn(Pos) -> String| -> Vec<String> {
            (oy..oy + level.height).map(|y| (ox..ox + level.width).map(|x| f((x, y))).collect()).collect()
        };
        let mut map = serde_json::json!({
            "level": level.id, "name": level.name_at(game.player().pos),
            "origin": [ox, oy], "width": level.width, "height": level.height,
            "stairs_up": level.stairs_up, "stairs_down": level.stairs_down,
            "towns": level.towns().map(|town| serde_json::json!({
                "name": town.name(), "origin": town.origin,
                "width": isle_core::Town::WIDTH, "height": isle_core::Town::HEIGHT,
                "start": town.start(), "shop": town.shop(), "temple": town.respawn(), "stairs_down": town.stairs(),
            })).collect::<Vec<_>>(),
        });
        match view.map {
            Map::Off => {}
            Map::Rows => {
                map["terrain"] = rows(&|pos| terrain(level.tile(pos), level.zone(pos)).to_string()).into();
                map["view"] = rows(&|pos| cell(game, pos, &labels).0).into();
            }
            Map::Cells => {
                let cells: Vec<Vec<serde_json::Value>> = (oy..oy + level.height)
                    .map(|y| {
                        (ox..ox + level.width)
                            .map(|x| {
                                let pos = (x, y);
                                serde_json::json!({
                                    "tile": format!("{:?}", level.tile(pos)),
                                    "zone": format!("{:?}", level.zone(pos)),
                                    "walkable": level.tile(pos).walkable(),
                                    "visible": game.sees(pos),
                                    "remembered": game.remembers(pos),
                                    "glyph": cell(game, pos, &labels).0,
                                })
                            })
                            .collect()
                    })
                    .collect();
                map["cells"] = cells.into();
            }
        }
        if !matches!(view.map, Map::Off) {
            report["map"] = map;
        }
        if view.game {
            // Through text, as the save file is, so genome f32s keep their short form rather than widened f64 digits.
            report["game"] =
                serde_json::from_str(&serde_json::to_string(game).expect("games serialize")).expect("and parse back");
        }
        report
    }
    fn submit(&mut self, cx: &mut Context<IsleView>) {
        let Some(line) = self.input.submit() else { return };
        if self.game.phase() != &Phase::Play {
            self.mark = self.game.log_total();
            self.game.answer(&line);
            self.started = Clock::now();
            self.save();
            return;
        }
        self.play_line(&line, cx);
    }

    fn play_line(&mut self, line: &str, cx: &mut Context<IsleView>) {
        self.show_skills = matches!(self.game.parse(line), Ok(Parsed::Instant(Instant::Skills)));
        let turn_based = self.round.is_none();
        if turn_based {
            self.mark = self.game.log_total();
        }
        let quit = submit_commands(&mut self.game, &mut self.queue, &mut self.paused, turn_based, line);
        self.save();
        if quit {
            cx.quit();
        }
    }

    /// Fraction of the round gone; always zero without a clock.
    fn clock(&self) -> f32 {
        self.round.map_or(0.0, |round| (self.started.elapsed().as_secs_f32() / round.as_secs_f32()).clamp(0.0, 1.0))
    }
}

/// Handles a play-mode line, returning whether the player asked to quit.
fn submit_commands(
    game: &mut Game,
    queue: &mut VecDeque<(String, Command)>,
    paused: &mut bool,
    turn_based: bool,
    line: &str,
) -> bool {
    let parts: Vec<&str> = line.split(';').map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut rounds_played = 0;
    for (i, part) in parts.iter().enumerate() {
        match part.to_lowercase().as_str() {
            "pause" if turn_based => game.note("There is no clock. The world waits for you."),
            "pause" => {
                *paused = !*paused;
                game.note(if *paused { "The clock stops." } else { "The clock runs again." });
            }
            "stop" => {
                queue.clear();
                game.note("You put aside your plans.");
            }
            "quit" => {
                return true;
            }
            _ => match game.parse(part) {
                Ok(Parsed::Instant(instant)) => game.instant(instant),
                Ok(Parsed::Round(_)) if rounds_played >= QUEUE || queue.len() >= QUEUE => {
                    game.note("You can plan only five rounds ahead.");
                    if turn_based {
                        break;
                    }
                }
                Ok(Parsed::Round(command)) if turn_based => {
                    rounds_played += 1;
                    if !play_round(game, command) && i + 1 < parts.len() {
                        game.note("You break off your plan.");
                        break;
                    }
                }
                Ok(Parsed::Round(command)) => queue.push_back(((*part).to_owned(), command)),
                Err(message) => game.note(message),
            },
        }
    }
    false
}

/// Plays one round of a chain. False if it went badly enough that the rest of the plan should wait: you
/// were hurt or fell, or something new came into sight.
fn play_round(game: &mut Game, command: Command) -> bool {
    let (mark, before) = (game.log_total(), sighted_ids(game));
    game.round(Some(command));
    !setback(game, mark, &before)
}

fn sighted_ids(game: &Game) -> Vec<u32> {
    game.sighted().iter().map(|s| s.creature.id).collect()
}

/// Whether, since the log stood at `mark`, you were hurt or fell, or something not in `before` came into sight.
fn setback(game: &Game, mark: u64, before: &[u32]) -> bool {
    let news = (game.log_total() - mark) as usize;
    let hurt = game.log().iter().rev().take(news).any(|line| matches!(line.tone, Tone::Hurt | Tone::Death));
    hurt || game.sighted().iter().any(|s| !before.contains(&s.creature.id))
}

/// One character per cell of raw terrain, as the map source draws it.
fn terrain(tile: Tile, zone: Zone) -> char {
    match (tile, zone) {
        (Tile::Floor, Zone::Shop) => 'S',
        (Tile::Floor, Zone::Temple) => 'H',
        (Tile::Wall, _) => '#',
        (Tile::Floor, _) => '.',
        (Tile::Grass, _) => ',',
        (Tile::Tree, _) => 'T',
        (Tile::Water, _) => '~',
        (Tile::Door, _) => '+',
        (Tile::StairsDown, _) => '>',
        (Tile::StairsUp, _) => '<',
        (Tile::Counter, _) => '=',
        (Tile::Altar, _) => '_',
        (Tile::Wild(id), _) => id.biome().glyph,
        (Tile::Rock, _) => '^',
    }
}
impl Focusable for IsleView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for IsleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<IsleView>) -> impl IntoElement {
        self.sprites.begin(window);
        self.art.begin(window, mood::of(&self.game, self.forced_mood));
        let elapsed = self.born.elapsed();
        let frame = ((elapsed.as_millis() as u64 / FRAME_MS) % u64::from(FRAMES)) as u8;
        let (map, animated) = map(&self.game, &mut self.sprites, &mut self.art, frame, elapsed.as_secs_f32());
        self.animated = animated;
        self.sprites.end(window);
        self.art.end(window);
        let title = self.save_error.clone().unwrap_or_else(|| {
            let file = self.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            format!("Isle · seed {} · {file}", self.game.seed())
        });
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.key(&event.keystroke, cx)))
            .size_full()
            .bg(rgb(SCREEN))
            .font_family(FONT)
            .text_size(px(14.0))
            .line_height(px(LINE))
            .text_color(rgb(TEXT))
            .px(px(GAP))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(TITLE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(rgb(if self.save_error.is_some() { 0xff_70_70 } else { FAINT }))
                    .child(title),
            )
            .child(div().flex().flex_row().gap(px(GAP)).child(map).child(self.panel()))
            .child(div().h(px(GAP)))
            .child(log(&self.game, self.mark))
            .child(div().h(px(GAP)))
            .child(self.prompt())
    }
}

impl IsleView {
    fn panel(&self) -> Div {
        let game = &self.game;
        let p = game.player();
        let mut panel = frame().w(px(PANEL)).h(px(MAP_H)).p(px(INSET)).flex().flex_col().overflow_hidden();
        if self.show_skills {
            return panel
                .child(heading("PRACTICE"))
                .children(
                    skill_lines(p)
                        .into_iter()
                        .map(|line| div().h(px(LINE)).flex_none().text_color(rgb(TEXT)).child(line)),
                )
                .child(rule())
                .child(dim_line("Practice grants power. XP records rank."))
                .child(dim_line("Weak threats stop teaching as you grow."))
                .child(dim_line("Escape or another command returns."));
        }
        if game.phase() != &Phase::Play {
            return panel
                .child(heading("A NEW ARRIVAL"))
                .child(dim_line("Type your answer below and press Enter."))
                .child(rule())
                .child(dim_line("No classes. What you do shapes you."))
                .child(dim_line("Fight, shoot, hide, or learn magic."))
                .child(dim_line("Practice grants power; XP records your rank."))
                .child(dim_line("Type skills to review your practice."));
        }
        panel = panel
            .child(heading_owned(p.name.clone()))
            .child(pair("Rank", level_text(p)))
            .child(bar("HP", p.hp, p.max_hp, HP))
            .child(bar("Focus", p.focus, p.max_focus, FOCUS));
        panel = panel
            .child(pair("Armor class", format!("{}", p.ac())))
            .child(pair("Gold", format!("{}", p.gold)))
            .child(rule())
            .child(pair("Hand", hand_text(p)))
            .child(pair("Bow", bow_text(p)))
            .child(pair("Worn", worn_text(p)))
            .child(dim_line_owned(practice_text(p)));
        if let Some(status) = demigod_status(p) {
            panel =
                panel.child(div().h(px(LINE)).flex_none().text_size(px(12.0)).text_color(rgb(SET_GLOW)).child(status));
        }
        if let Some(relic) = p.relic {
            panel = panel.child(pair("Relic", relic.name().into()));
            for line in wrap(relic.description(), 36) {
                panel = panel.child(dim_line_owned(line));
            }
        }
        if let Some(states) = states_text(game) {
            panel = panel.child(div().h(px(LINE)).text_color(rgb(HEADING)).child(states));
        }
        panel = panel.child(rule()).child(pair("Place", place_text(game)));
        let Some(round) = self.round else {
            return self.sighted(panel.child(pair("Round", format!("{}", game.round_number() + 1))));
        };
        let clock = self.clock_text(round);
        panel = panel.child(gauge(&format!("Round {}", game.round_number() + 1), self.clock(), CLOCK, &clock));
        let mut planned = self.queue.iter().map(|(text, _)| text.clone());
        panel = panel.child(pair("Next", planned.next().unwrap_or_else(|| "stand ready".into())));
        let later: Vec<String> = planned.collect();
        if !later.is_empty() {
            panel = panel.child(pair("Then", later.join("; ")));
        }
        self.sighted(panel)
    }

    /// The creatures in sight, lettered as commands name them.
    fn sighted(&self, mut panel: Div) -> Div {
        let game = &self.game;
        let p = game.player();
        panel = panel.child(rule()).child(heading("IN SIGHT"));
        let mut list = div().id("sighted").flex_1().min_h_0().flex().flex_col().overflow_y_scroll();
        let sighted = game.sighted();
        if sighted.is_empty() {
            list = list.child(dim_line("Nothing stirs."));
        }
        for s in sighted.iter().take(if sighted.iter().any(|s| s.creature.wild.is_some()) { 3 } else { 6 }) {
            let c = s.creature;
            let stats = c.stats();
            if c.phenotype().is_some() || c.kind == Kind::CinderWarden {
                list = list
                    .child(
                        div()
                            .h(px(LINE))
                            .flex_none()
                            .text_color(rgb(creature_color(c.kind, c.hunting)))
                            .child(format!("{} {}", s.label, stats.name)),
                    )
                    .children(detail_sight_lines(p, c).into_iter().map(|line| {
                        div().min_h(px(LINE)).flex_none().text_size(px(12.0)).text_color(rgb(DIM)).child(line)
                    }));
                continue;
            }
            list = list.child(
                div()
                    .h(px(LINE))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .whitespace_nowrap()
                    .child(
                        div().w(px(14.0)).text_color(rgb(creature_color(c.kind, c.hunting))).child(s.label.to_string()),
                    )
                    .child(div().text_color(rgb(TEXT)).child(stats.name))
                    .child(div().text_color(rgb(DIM)).child(sight_notes(p, c))),
            );
        }
        panel.child(list)
    }

    /// The window's contents as text: the map, the panel, the log, and the prompt.
    fn screen(&self) -> serde_json::Value {
        let game = &self.game;
        let p = game.player();
        let me = p.pos;
        let labels: Vec<(Pos, char)> = game.sighted().iter().map(|s| (s.creature.pos, s.label)).collect();
        let view: Vec<String> = (-SIGHT..=SIGHT)
            .map(|dy| (-SIGHT..=SIGHT).map(|dx| cell(game, (me.0 + dx, me.1 + dy), &labels).0).collect())
            .collect();
        let mut panel = Vec::new();
        if game.phase() == &Phase::Play {
            panel.push(p.name.clone());
            panel.push(format!("Rank {}", level_text(p)));
            panel.push(format!("HP {}/{}", p.hp, p.max_hp));
            panel.push(format!("Focus {}/{}", p.focus, p.max_focus));
            panel.push(format!("Armor class {}", p.ac()));
            panel.push(format!("Gold {}", p.gold));
            panel.push(format!("Hand {}", hand_text(p)));
            panel.push(format!("Bow {}", bow_text(p)));
            panel.push(format!("Worn {}", worn_text(p)));
            panel.push(practice_text(p));
            if let Some(status) = demigod_status(p) {
                panel.push(status.into());
            }
            if let Some(relic) = p.relic {
                panel.push(format!("Relic {}", relic.name()));
                panel.extend(wrap(relic.description(), 36));
            }
            panel.extend(states_text(game));
            panel.push(format!("Place {}", place_text(game)));
            let number = game.round_number() + 1;
            match self.round {
                None => panel.push(format!("Round {number}")),
                Some(round) => {
                    panel.push(format!("Round {number} · {}", self.clock_text(round)));
                    let planned: Vec<&str> = self.queue.iter().map(|(text, _)| text.as_str()).collect();
                    panel.push(format!("Next {}", planned.first().copied().unwrap_or("stand ready")));
                    if planned.len() > 1 {
                        panel.push(format!("Then {}", planned[1..].join("; ")));
                    }
                }
            }
            panel.push("IN SIGHT".into());
            let sighted = game.sighted();
            if sighted.is_empty() {
                panel.push("Nothing stirs.".into());
            }
            for s in sighted.iter().take(if sighted.iter().any(|s| s.creature.wild.is_some()) { 3 } else { 6 }) {
                if s.creature.phenotype().is_some() || s.creature.kind == Kind::CinderWarden {
                    panel.push(format!("{} {}", s.label, s.creature.stats().name));
                    panel.extend(detail_sight_lines(p, s.creature));
                } else {
                    panel.push(format!("{} {} {}", s.label, s.creature.stats().name, sight_notes(p, s.creature)));
                }
            }
        } else {
            panel.push("A NEW ARRIVAL".into());
        }
        if self.show_skills {
            panel = vec!["PRACTICE".into()];
            panel.extend(skill_lines(p));
            panel.extend([
                "Practice grants power. XP records rank.".into(),
                "Weak threats stop teaching as you grow.".into(),
                "Escape or another command returns.".into(),
            ]);
        }
        let log = game.log();
        let (before, after) = self.input.halves();
        serde_json::json!({
            "view": view,
            "panel": panel,
            "log": log[log.len().saturating_sub(LOG_LINES)..].iter().map(|l| &l.text).collect::<Vec<_>>(),
            "prompt": {"text": format!("{before}{after}"), "cursor": before.len()},
            "save_error": self.save_error,
        })
    }

    fn clock_text(&self, round: Duration) -> String {
        if self.paused { "PAUSED".to_owned() } else { format!("{:.0}s", round.as_secs_f32() * (1.0 - self.clock())) }
    }

    fn prompt(&self) -> Div {
        let creating = self.game.phase() != &Phase::Play;
        let hint = if creating { "your answer" } else { "type a command (help lists them)" };
        let (before, after) = self.input.halves();
        let cursor = div().flex_none().w(px(2.0)).h(px(18.0)).bg(rgb(HEADING));
        frame()
            .w_full()
            .h(px(PROMPT))
            .px(px(INSET))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .text_size(px(16.0))
            .child(div().text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(">"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .whitespace_nowrap()
                    .child(div().child(SharedString::from(before.to_owned())))
                    .child(cursor)
                    .child(div().child(SharedString::from(after.to_owned()))),
            )
            .when(self.input.is_empty(), |d| d.child(div().text_color(rgb(FAINT)).child(hint)))
    }
}

/// The two characters, color, and background a cell shows.
fn cell(game: &Game, pos: Pos, labels: &[(Pos, char)]) -> (String, u32, Option<u32>) {
    let me = game.player().pos;
    let visible = game.sees(pos);
    if !visible && !game.remembers(pos) {
        return ("  ".into(), 0, None);
    }
    let level = game.level();
    let back = match level.zone(pos) {
        Zone::Shop => Some(0x1a_14_0a),
        Zone::Temple => Some(0x10_12_20),
        Zone::Open => (visible && game.illuminated(pos)).then_some(CAVE_LIGHT),
    };
    if pos == me {
        return ("@ ".into(), YOU, back);
    }
    if visible {
        if let Some(c) = game.creature_at(pos) {
            let label = labels.iter().find(|(p, _)| *p == pos).map_or('?', |&(_, l)| l);
            return (format!("{label}{}", initial(c.kind)), creature_color(c.kind, c.hunting), back);
        }
        if let Some(pile) = game.pile_at(pos) {
            let (glyph, color) = match pile.items.first() {
                Some(Item::Gold(_)) if pile.items.len() == 1 => ("$$", 0xf0_d0_40),
                Some(Item::Arrows(_)) if pile.items.len() == 1 => ("//", 0xc8_a0_70),
                _ => ("()", 0x6a_d0_d0),
            };
            return (glyph.into(), color, back);
        }
    }
    if let Some(site) = game.landmark_at(pos) {
        let color = if visible { HEADING } else { dimmed(HEADING) };
        let glyph = match site.kind {
            LandmarkKind::CinderWatch => {
                if site.opened {
                    "W."
                } else if game.cinder_watch().is_some_and(|watch| watch.phase == isle_core::WatchPhase::Cleared) {
                    "W?"
                } else {
                    "W!"
                }
            }
            LandmarkKind::AshenForge => {
                if site.opened {
                    "A."
                } else if covenant_site_cleared(game, pos) {
                    "A?"
                } else {
                    "A!"
                }
            }
            LandmarkKind::WellOfNames => {
                if site.opened {
                    "N."
                } else if covenant_site_cleared(game, pos) {
                    "N?"
                } else {
                    "N!"
                }
            }
            _ => return (if site.opened { "&." } else { "&?" }.into(), color, back),
        };
        return (glyph.into(), color, back);
    }
    if game.herb_at(pos).is_some_and(|patch| !patch.picked) {
        let color = 0x9a_c8_76;
        return ("h*".into(), if visible { color } else { dimmed(color) }, back);
    }
    let (glyph, color) = match level.tile(pos) {
        Tile::Wall => ("[]", 0x9a_9a_a8),
        Tile::Floor => (". ", 0x5c_5f_6e),
        Tile::Grass => (", ", 0x4f_8a_3c),
        Tile::Tree => ("^^", 0x3f_a0_4a),
        Tile::Water => ("~~", 0x4a_7a_d0),
        Tile::Door => ("++", 0xc0_8a_40),
        Tile::StairsDown => (">>", 0xff_ff_ff),
        Tile::StairsUp => ("<<", 0xff_ff_ff),
        Tile::Counter => ("==", 0xa0_70_40),
        Tile::Altar => ("**", 0xf0_d0_80),
        Tile::Rock => ("##", 0x9a_9a_a8),
        Tile::Wild(id) => {
            let biome = id.biome();
            let [r, g, b] = biome.color;
            let color = u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
            return (format!("{} ", biome.glyph), if visible { color } else { dimmed(color) }, back);
        }
    };
    (glyph.into(), if visible { color } else { dimmed(color) }, back)
}

fn level_text(p: &Player) -> String {
    match p.next_level() {
        Some(next) => format!("{} · {}/{next} xp", p.level, p.xp),
        None => format!("{} · {} xp", p.level, p.xp),
    }
}

fn hand_text(p: &Player) -> String {
    format!("{} · {}", p.weapon.name(), p.practice.get(Practice::weapon(p.weapon)).rank().name())
}

fn skill_lines(p: &Player) -> Vec<String> {
    Practice::ALL
        .into_iter()
        .map(|practice| format!("{}: {}", practice.name(), p.practice.get(practice).rank().name()))
        .collect()
}

fn practice_text(p: &Player) -> String {
    format!(
        "Focus {} · stealth {}",
        p.practice.get(Practice::Focus).rank().name(),
        p.practice.get(Practice::Stealth).rank().name(),
    )
}

fn demigod_status(p: &Player) -> Option<&'static str> {
    p.is_demigod().then_some("DEMIGOD · EXILE SET")
}

fn covenant_site_cleared(game: &Game, pos: Pos) -> bool {
    game.covenant().trials.iter().any(|trial| trial.pos == pos && trial.phase == isle_core::WatchPhase::Cleared)
}

fn bow_text(p: &Player) -> String {
    match p.bow {
        Some(bow) => format!("{} · {} arrows{}", bow.name(), p.arrows, if p.nocked { " · NOCKED" } else { "" }),
        None => "none".into(),
    }
}

fn worn_text(p: &Player) -> String {
    match (p.armor, p.ring) {
        (Some(a), true) => format!("{} · ring", a.name()),
        (Some(a), false) => a.name().into(),
        (None, true) => "ring of warding".into(),
        (None, false) => "nothing".into(),
    }
}

fn states_text(game: &Game) -> Option<String> {
    let p = game.player();
    let mut states = Vec::new();
    if let Some(spell) = p.chanted {
        states.push(format!("chanted {}", spell.name()));
    }
    if p.shield > 0 {
        states.push(format!("shield {}", p.shield));
    }
    if p.hidden {
        states.push("hidden".into());
    }
    if let Some(readied) = p.readied {
        states.push(readied.name().into());
    }
    if game.illuminated(p.pos) {
        states.push("lit · no hiding".into());
    }
    (!states.is_empty()).then(|| states.join(" · "))
}

fn place_text(game: &Game) -> String {
    match game.zone() {
        Zone::Shop => format!("{} · shop", game.level().name_at(game.player().pos)),
        Zone::Temple => format!("{} · temple", game.level().name_at(game.player().pos)),
        Zone::Open => {
            let name = game.level().name_at(game.player().pos).to_owned();
            if matches!(game.player().at, isle_core::LevelId::Deep(..)) {
                format!("{name} · {}", if game.conquered(game.player().at) { "conquered" } else { "unconquered" })
            } else {
                name
            }
        }
    }
}

fn sight_notes(p: &Player, c: &Creature) -> String {
    let stats = c.stats();
    let way = isle_core::Dir::from_delta(c.pos.0 - p.pos.0, c.pos.1 - p.pos.1).map_or("", isle_core::Dir::short);
    let mut notes = vec![format!("{} {way}", distance(p.pos, c.pos))];
    if c.hp < stats.hp {
        notes.push(if c.hp * 3 <= stats.hp { "badly hurt".into() } else { "hurt".into() });
    }
    if c.stunned > 0 {
        notes.push("stunned".into());
    }
    if c.hunting {
        notes.push("hunting".into());
    }
    notes.join(" · ")
}

/// Shared by the visual panel and text inspection, so cues cannot drift between them.
fn detail_sight_lines(p: &Player, c: &Creature) -> Vec<String> {
    let mut lines = vec![format!("  {}", sight_notes(p, c))];
    if let Some(phenotype) = c.phenotype() {
        lines[0].push_str(&format!(" · {}", phenotype.temperament.word()));
    }
    if c.kind == Kind::CinderWarden {
        lines.push("  arrows halved · Spark ignores plating".into());
    }
    let traits = c.cave_traits();
    if traits.tremorsense {
        lines.push("  tremorsense · quiet steps help".into());
    }
    if traits.luminous {
        lines.push("  luminous · spoils nearby hiding".into());
    }
    lines
}

fn initial(kind: Kind) -> char {
    match kind {
        Kind::Rat => 'r',
        Kind::Wolf => 'w',
        Kind::Goblin => 'g',
        Kind::GoblinArcher => 'a',
        Kind::Orc => 'o',
        Kind::Skeleton => 's',
        Kind::Troll => 'T',
        Kind::CinderWarden => 'C',
        Kind::AshSentry => 'a',
        Kind::AshColossus => 'X',
        Kind::HollowCantor => 'h',
        Kind::Wild => '*',
    }
}

fn creature_color(kind: Kind, hunting: bool) -> u32 {
    match (kind, hunting) {
        (Kind::Troll, _) => 0xe0_70_ff,
        (_, true) => 0xff_5a_4a,
        (_, false) => 0xe0_a0_60,
    }
}

fn dimmed(color: u32) -> u32 {
    let scale = |shift: u32| (((color >> shift) & 0xff) * 35 / 100) << shift;
    scale(16) | scale(8) | scale(0)
}

#[derive(Clone, Copy)]
enum MapActor<'a> {
    Player,
    Creature { creature: &'a Creature, label: char },
    Item(Object),
}

struct MapCellArt<'a> {
    terrain: TerrainArt,
    ground: GroundCell,
    remembered: bool,
    lit: bool,
    actor: Option<MapActor<'a>>,
}

/// The visual map's shared selection helper. Unknown cells are blank; memory carries terrain only.
#[cfg(test)]
fn map_cell_art<'a>(game: &'a Game, pos: Pos, labels: &[(Pos, char)]) -> Option<MapCellArt<'a>> {
    map_cell_art_in(game, &|at| game.sees(at), pos, labels)
}

/// [`map_cell_art`] with sight answered by `sees`, which must agree with [`Game::sees`].
fn map_cell_art_in<'a>(
    game: &'a Game,
    sees: &dyn Fn(Pos) -> bool,
    pos: Pos,
    labels: &[(Pos, char)],
) -> Option<MapCellArt<'a>> {
    let visible = sees(pos);
    let remembered = !visible;
    if remembered && !game.remembers(pos) {
        return None;
    }
    let level = game.level();
    let actor = if pos == game.player().pos {
        Some(MapActor::Player)
    } else if visible {
        if let Some(creature) = game.creature_at(pos) {
            let label = labels.iter().find(|(at, _)| *at == pos).map_or('?', |&(_, label)| label);
            Some(MapActor::Creature { creature, label })
        } else {
            game.pile_at(pos).and_then(|pile| item_art(&pile.items)).map(MapActor::Item)
        }
    } else {
        None
    };
    let deep = level.deep().is_some();
    let mut terrain = terrain_art(level.tile(pos), level.ground(pos), deep, pos);
    if let Some(site) = game.landmark_at(pos) {
        terrain.object = Some(match site.kind {
            LandmarkKind::Cairn => Object::Cairn,
            LandmarkKind::Camp => Object::Camp,
            LandmarkKind::Shrine => Object::Shrine,
            LandmarkKind::CinderWatch => Object::Watchtower {
                cleared: game.cinder_watch().is_some_and(|watch| watch.phase == isle_core::WatchPhase::Cleared),
            },
            LandmarkKind::AshenForge => Object::AshenForge,
            LandmarkKind::WellOfNames => Object::WellOfNames,
        });
    }
    if let Some(patch) = game.herb_at(pos).filter(|patch| !patch.picked) {
        terrain.object = Some(Object::Herb(patch.herb));
    }
    // Unknown neighbors take this cell's own ground and tint, so edges never reveal unexplored terrain.
    let near = |dx: i32, dy: i32| {
        let at = (pos.0 + dx, pos.1 + dy);
        if (dx, dy) == (0, 0) || !(sees(at) || game.remembers(at)) {
            terrain
        } else {
            terrain_art(level.tile(at), level.ground(at), deep, at)
        }
    };
    let ground = GroundCell {
        pos,
        around: [-2, -1, 0, 1, 2].map(|dy| [-2, -1, 0, 1, 2].map(|dx| near(dx, dy).ground)),
        tints: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| near(dx, dy).tint)),
        styles: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| near(dx, dy).style)),
        flavors: [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| near(dx, dy).flavor)),
    };
    Some(MapCellArt { terrain, ground, remembered, lit: visible && game.illuminated(pos), actor })
}

/// How dark each cell around `pos` is drawn, rows north to south: 0 in sight, 1 remembered, 2 unknown.
fn sight_levels(game: &Game, sees: &dyn Fn(Pos) -> bool, pos: Pos) -> [[u8; 3]; 3] {
    [-1, 0, 1].map(|dy| {
        [-1, 0, 1].map(|dx| {
            let at = (pos.0 + dx, pos.1 + dy);
            if sees(at) {
                0
            } else if game.remembers(at) {
                1
            } else {
                2
            }
        })
    })
}

/// The map, and whether anything in it moves. `step` advances the grass sway and water glitter; `elapsed`
/// seconds drive glows and drifting mist.
fn map(game: &Game, sprites: &mut SpriteCache, art: &mut ArtCache, step: u8, elapsed: f32) -> (Div, bool) {
    // Mist drifts a pixel every 0.9 seconds.
    let drift = (elapsed / 0.9) as u32;
    let me = game.player().pos;
    let mut animated = false;
    let labels: Vec<(Pos, char)> = game.sighted().iter().map(|s| (s.creature.pos, s.label)).collect();
    // Every cell asks about its neighbors' sight too; cast each cell's rays once per frame.
    const REACH: i32 = SIGHT + 2;
    let seen: Vec<bool> = (-REACH..=REACH)
        .flat_map(|dy| (-REACH..=REACH).map(move |dx| (me.0 + dx, me.1 + dy)))
        .map(|pos| game.sees(pos))
        .collect();
    let sees = |pos: Pos| {
        let (dx, dy) = (pos.0 - me.0, pos.1 - me.1);
        if dx.abs() <= REACH && dy.abs() <= REACH {
            seen[((dy + REACH) * (2 * REACH + 1) + dx + REACH) as usize]
        } else {
            game.sees(pos)
        }
    };
    let cells: Vec<Option<MapCellArt>> = (-SIGHT..=SIGHT)
        .flat_map(|dy| (-SIGHT..=SIGHT).map(move |dx| (me.0 + dx, me.1 + dy)))
        .map(|pos| map_cell_art_in(game, &sees, pos, &labels))
        .collect();
    let at = |dx: i32, dy: i32| {
        (dx.abs() <= SIGHT && dy.abs() <= SIGHT)
            .then(|| cells[((dy + SIGHT) * VIEW + dx + SIGHT) as usize].as_ref())
            .flatten()
    };
    let image_size = art.display_size();
    let (object_w, object_h, object_top) = art.object_frame();
    let layer = |image| img(image).size(px(image_size)).absolute().top_0().left_0();
    let tall = |image| img(image).w(px(object_w)).h(px(object_h)).absolute().top(px(object_top)).left_0();
    let cell = || div().w(px(CELL_W)).h(px(CELL_H)).flex_none().relative();
    // Four stacked grids, so a tall crown covers the cell above it, the fog covers crowns as it covers the
    // ground, and letters stay legible over both.
    let (mut grounds, mut scenes, mut shades, mut marks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for dy in -SIGHT..=SIGHT {
        let (mut ground_row, mut scene_row, mut shade_row, mut mark_row) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for dx in -SIGHT..=SIGHT {
            let pos = (me.0 + dx, me.1 + dy);
            let Some(cell_art) = at(dx, dy) else {
                ground_row.push(cell().bg(rgb(SCREEN)));
                scene_row.push(cell());
                // Unknown land stays blank, even where a known tree's crown would rise into it.
                shade_row.push(cell().bg(rgb(SCREEN)));
                mark_row.push(cell());
                continue;
            };
            let mut ground = cell().bg(rgb(SCREEN)).child(layer(art.ground(cell_art.ground, cell_art.lit)));
            let visible = !cell_art.remembered;
            if let Some(tufts) = art.tufts(cell_art.ground, step, cell_art.remembered, cell_art.lit) {
                animated |= visible;
                ground = ground.child(layer(tufts));
            }
            if let Some(shimmer) = art.shimmer(cell_art.ground, step, cell_art.remembered, cell_art.lit) {
                animated |= visible;
                ground = ground.child(layer(shimmer));
            }
            if let Some(heat) = visible.then(|| art.heat(cell_art.ground, cell_art.lit)).flatten() {
                animated = true;
                let glow = 0.35 + 0.65 * mood::pulse(elapsed, 2.6);
                ground = ground.child(layer(heat).opacity(glow));
            }
            if cell_art.lit {
                // Living light breathes; every lit cell shares the phase, so the glow has no seams.
                animated = true;
                let alpha = (10.0 + 26.0 * mood::pulse(elapsed, 1.9)) as u32;
                ground = ground.child(
                    div().absolute().top_0().left_0().w(px(CELL_W)).h(px(CELL_H)).bg(rgba(0xa8_f0_d0_00 | alpha)),
                );
            }
            if let Some(mist) = art.mist(pos, if visible { drift } else { 0 }) {
                animated |= visible;
                ground = ground.child(layer(mist));
            }
            ground_row.push(ground);

            let mut scene = cell();
            let mut mark = cell();
            let mut marker = None;
            if let Some(object) = cell_art.terrain.object {
                let image = tall(art.object(&cell_art.terrain, object, cell_art.lit));
                // A crown never hides whoever stands just north of it; it thins to show them.
                let veils = at(dx, dy - 1).is_some_and(|north| north.actor.is_some());
                scene = scene.child(if veils && art.rises(&cell_art.terrain, object) {
                    image.opacity(0.45)
                } else {
                    image
                });
                marker = match object {
                    Object::StairsUp => Some("<"),
                    Object::StairsDown => Some(">"),
                    Object::Watchtower { cleared: false } => Some("!"),
                    Object::Watchtower { cleared: true } if game.landmark_at(pos).is_some_and(|site| !site.opened) => {
                        Some("?")
                    }
                    Object::Cairn | Object::Camp | Object::Shrine
                        if game.landmark_at(pos).is_some_and(|site| !site.opened) =>
                    {
                        Some("?")
                    }
                    Object::AshenForge | Object::WellOfNames
                        if game.landmark_at(pos).is_some_and(|site| !site.opened) =>
                    {
                        Some(if covenant_site_cleared(game, pos) { "?" } else { "!" })
                    }
                    _ => None,
                };
            }
            match cell_art.actor {
                Some(MapActor::Player) => {
                    scene = scene.child(layer(art.hero(Look::of(game.player()), cell_art.lit)));
                }
                Some(MapActor::Creature { creature, label }) if sprite_creature(game, pos).is_some() => {
                    let color = creature_color(creature.kind, creature.hunting);
                    if let Some(image) = sprites.get(creature) {
                        let display_size = sprites.display_size();
                        let inset = (CELL_W - display_size) / 2.0;
                        scene =
                            scene.child(img(image).size(px(display_size)).absolute().top(px(inset)).left(px(inset)));
                        mark = mark.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w(px(12.0))
                                .h(px(14.0))
                                .bg(rgb(SCREEN))
                                .text_color(rgb(color))
                                .text_size(px(12.0))
                                .line_height(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .child(label.to_string()),
                        );
                    } else {
                        mark = creature_glyph(mark, creature, label);
                    }
                }
                Some(MapActor::Creature { creature, label }) => {
                    if let Some(object) = match creature.kind {
                        Kind::CinderWarden => Some(Object::CinderWarden),
                        Kind::AshSentry => Some(Object::AshSentry),
                        _ => None,
                    } {
                        scene = scene.child(tall(art.item(object)));
                        mark = mark.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .w(px(12.0))
                                .h(px(14.0))
                                .bg(rgb(SCREEN))
                                .text_color(rgb(creature_color(creature.kind, creature.hunting)))
                                .text_size(px(12.0))
                                .line_height(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .child(label.to_string()),
                        );
                    } else {
                        mark = creature_glyph(mark, creature, label);
                    }
                }
                Some(MapActor::Item(object)) => {
                    scene = scene.child(tall(art.item(object)));
                }
                None => {}
            }
            scene_row.push(scene);
            // Remembered land is dimmed over its objects as well as its ground, so the edge of sight is soft
            // for both.
            shade_row.push(match art.shade(pos, sight_levels(game, &sees, pos)) {
                Some(shade) => cell().child(layer(shade)),
                None => cell(),
            });
            // Stairs stay legible even under the hero who just climbed them.
            if let Some(marker) = marker {
                mark = mark.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(px(12.0))
                        .h(px(14.0))
                        .bg(rgba(0x071013d0))
                        .text_color(rgb(0xff_ff_ff))
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .child(marker),
                );
            }
            mark_row.push(mark);
        }
        grounds.push(ground_row);
        scenes.push(scene_row);
        shades.push(shade_row);
        marks.push(mark_row);
    }
    let grid = |rows: Vec<Vec<Div>>| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .flex()
            .flex_col()
            .children(rows.into_iter().map(|row| div().flex().flex_row().children(row)))
    };
    let side = VIEW as f32 * CELL_W;
    let layers = div()
        .relative()
        .w(px(side))
        .h(px(VIEW as f32 * CELL_H))
        .child(grid(grounds))
        .child(grid(scenes))
        .child(grid(shades))
        .child(grid(marks));
    // Tall crowns in the top row rise into the frame's border; clip them there.
    (frame().w(px(MAP_W)).h(px(MAP_H)).overflow_hidden().child(layers), animated)
}
fn creature_glyph(base: Div, creature: &Creature, label: char) -> Div {
    let color = creature_color(creature.kind, creature.hunting);
    base.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .w(px(CELL_W))
            .h(px(CELL_H))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(GLYPH))
            .text_color(rgb(color))
            .child(format!("{label}{}", initial(creature.kind))),
    )
}

/// Match the text map's visibility and player priority; remembered creatures never become ghost sprites.
fn sprite_creature(game: &Game, pos: Pos) -> Option<&Creature> {
    (pos != game.player().pos && game.sees(pos))
        .then(|| game.creature_at(pos))
        .flatten()
        .filter(|creature| creature.wild.is_some())
}

const fn tint(tone: Tone, new: bool) -> u32 {
    match (tone, new) {
        (Tone::Info, true) => TEXT,
        (Tone::Info, false) => DIM,
        (Tone::Hit, true) => 0xf0_e0_a0,
        (Tone::Hit, false) => 0x80_78_58,
        (Tone::Miss, true) => 0xa8_a8_b8,
        (Tone::Miss, false) => 0x5a_5a_66,
        (Tone::Hurt, true) => 0xff_6a_5a,
        (Tone::Hurt, false) => 0x8c_44_3e,
        (Tone::Kill, true) => 0x8c_e0_6a,
        (Tone::Kill, false) => 0x4e_7a_40,
        (Tone::Death, true) => 0xe0_70_ff,
        (Tone::Death, false) => 0x7a_48_8a,
        (Tone::Gain, true) => HEADING,
        (Tone::Gain, false) => 0x86_70_3a,
        (Tone::System, true) => 0x8c_b8_e8,
        (Tone::System, false) => 0x4a_60_7a,
    }
}

/// Log characters that fit across one row: Menlo's advance is 0.602 of its size.
const LOG_CHARS: usize = ((GAP + MAP_W + GAP + PANEL - 2.0 * (INSET + BORDER)) / (LOG_SIZE * 0.602)) as usize;
const LOG_SIZE: f32 = 14.0;

/// Splits `text` into rows of at most `width` characters, breaking at spaces where it can.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in text.split(' ') {
        let mut word = word.to_owned();
        while word.chars().count() > width {
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
            }
            let rest = word.split_off(word.char_indices().nth(width).map_or(word.len(), |(i, _)| i));
            rows.push(std::mem::replace(&mut word, rest));
        }
        let needed = row.chars().count() + usize::from(!row.is_empty()) + word.chars().count();
        if needed > width && !row.is_empty() {
            rows.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(&word);
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

fn log(game: &Game, mark: u64) -> Div {
    let lines = game.log();
    let shown = &lines[lines.len().saturating_sub(LOG_LINES)..];
    let first = game.log_total() - shown.len() as u64;
    let mut rows: Vec<Div> = Vec::new();
    for (i, line) in shown.iter().enumerate() {
        let new = first + i as u64 >= mark;
        for (k, text) in wrap(&line.text, LOG_CHARS - 2).into_iter().enumerate() {
            let text = if k == 0 { text } else { format!("  {text}") };
            rows.push(
                div()
                    .h(px(LINE))
                    .flex_none()
                    .whitespace_nowrap()
                    .text_color(rgb(tint(line.tone, new)))
                    .when(new && line.tone == Tone::Death, |d| d.font_weight(FontWeight::BOLD))
                    .child(SharedString::from(text)),
            );
        }
    }
    let rows = rows.split_off(rows.len().saturating_sub(LOG_LINES));
    frame()
        .w_full()
        .h(px(LOG_H))
        .p(px(INSET))
        .text_size(px(LOG_SIZE))
        .flex()
        .flex_col()
        .justify_end()
        .overflow_hidden()
        .children(rows)
}

fn frame() -> Div {
    div().flex_none().bg(rgb(SCREEN)).border(px(BORDER)).border_color(rgb(FRAME))
}

fn heading(text: &'static str) -> Div {
    div().h(px(LINE)).flex_none().text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(text)
}

fn heading_owned(text: String) -> Div {
    div().h(px(LINE)).flex_none().text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(SharedString::from(text))
}

fn dim_line(text: &'static str) -> Div {
    div().h(px(LINE)).flex_none().text_size(px(12.0)).text_color(rgb(DIM)).child(text)
}

fn dim_line_owned(text: String) -> Div {
    div().h(px(LINE)).flex_none().text_size(px(12.0)).text_color(rgb(DIM)).child(text)
}

fn rule() -> Div {
    div().my(px(5.0)).h(px(1.0)).flex_none().w_full().bg(rgb(FAINT))
}

fn pair(label: &'static str, value: String) -> Div {
    div()
        .h(px(LINE))
        .flex_none()
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(8.0))
        .child(div().flex_none().text_color(rgb(DIM)).child(label))
        .child(div().min_w_0().truncate().text_color(rgb(TEXT)).child(SharedString::from(value)))
}

fn bar(label: &'static str, value: i32, max: i32, fill: u32) -> Div {
    let fraction = value.max(0) as f32 / max.max(1) as f32;
    let low = value * 3 <= max;
    gauge(label, fraction, fill, &format!("{value}/{max}")).text_color(rgb(if low { HP } else { TEXT }))
}

fn gauge(label: &str, fraction: f32, fill: u32, value: &str) -> Div {
    const WIDTH: f32 = 130.0;
    div()
        .h(px(LINE))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(div().w(px(80.0)).text_color(rgb(DIM)).child(SharedString::from(label.to_owned())))
        .child(
            div()
                .w(px(WIDTH))
                .h(px(10.0))
                .bg(rgb(EMPTY_BAR))
                .child(div().h_full().w(px(WIDTH * fraction.clamp(0.0, 1.0))).bg(rgb(fill))),
        )
        .child(div().flex_grow())
        .child(div().child(SharedString::from(value.to_owned())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn made(seed: u64) -> Game {
        let mut game = Game::new(seed);
        game.answer("Ash");
        game
    }

    fn submit(game: &mut Game, line: &str) {
        let mut queue = VecDeque::new();
        let mut paused = false;
        assert!(!submit_commands(game, &mut queue, &mut paused, true, line));
        assert!(queue.is_empty() && !paused);
    }

    /// Resume a save with a single encounter and no scheduled spawns.
    fn encounter(seed: u64, kind: Kind, offset: i32, hp: i32) -> Game {
        let game = made(seed);
        let (x, y) = game.player().pos;
        let mut save = serde_json::to_value(&game).unwrap();
        let id = save["next_id"].clone();
        save["next_id"] = serde_json::json!(id.as_u64().unwrap() + 1);
        save["creatures"] = serde_json::json!([{
            "id": id, "kind": kind, "wild": null, "energy": 0, "at": "Surface",
            "pos": [x + offset, y], "home": [x + offset, y], "hp": kind.stats().hp,
            "hunting": false, "last_seen": null, "lost": 0, "stunned": 0, "spawn": null,
        }]);
        save["spawns"] = serde_json::json!([]);
        save["player"]["hp"] = serde_json::json!(hp);
        serde_json::from_value(save).unwrap()
    }

    #[test]
    fn commands_resolve_immediately_but_information_and_errors_take_no_time() {
        let mut game = made(7);
        let pos = game.player().pos;
        submit(&mut game, "e e; s");
        assert_eq!(game.round_number(), 2);
        assert_eq!(game.player().pos, (pos.0 + 2, pos.1 + 1));
        let creatures = game.creatures().to_vec();
        submit(&mut game, "look; inventory; skills; help; list; nonsense; pause; stop");
        assert_eq!(game.round_number(), 2);
        assert_eq!(game.creatures(), creatures);
        submit(&mut game, "wait; wait; wait; wait; wait; wait");
        assert_eq!(game.round_number(), 7, "at most five rounds per line");
    }

    #[test]
    fn arrow_movement_uses_the_game_rules_without_submitting_or_changing_a_draft() {
        let mut game = made(7);
        let pos = game.player().pos;
        let mut input = CommandLine::default();
        input.key(&gpui::Keystroke { key_char: Some("look".into()), ..Default::default() }, true);
        input.key(&gpui::Keystroke::parse("ctrl-a").unwrap(), true);
        for (clocked, stroke) in [(false, "right"), (true, "down")] {
            let InputAction::Move(dir) = input.key(&gpui::Keystroke::parse(stroke).unwrap(), true) else {
                panic!("an arrow moves");
            };
            let mut queue = VecDeque::new();
            submit_commands(&mut game, &mut queue, &mut false, !clocked, dir.short());
            assert_eq!(input.halves(), ("", "look"));
            if clocked {
                assert_eq!(queue.len(), 1);
                game.round(Some(queue.pop_front().unwrap().1));
            } else {
                assert!(queue.is_empty());
            }
        }
        assert_eq!(game.player().pos, (pos.0 + 1, pos.1 + 1));
        assert_eq!(game.round_number(), 2);
        assert_eq!(input.submit().as_deref(), Some("look"));
        assert_eq!(input.submit().as_deref(), Some("look"));
    }

    #[test]
    fn punctuation_shortcuts_use_stair_and_pickup_rules_in_both_modes() {
        for clocked in [false, true] {
            let mut save = serde_json::to_value(made(7)).unwrap();
            let stairs = made(7).level().stairs_down.unwrap();
            save["player"]["pos"] = serde_json::json!(stairs);
            save["creatures"] = serde_json::json!([]);
            save["spawns"] = serde_json::json!([]);
            save["piles"] = serde_json::to_value([isle_core::Pile {
                at: isle_core::LevelId::Surface,
                pos: stairs,
                items: vec![Item::Gold(9), Item::Arrows(3), Item::Potion],
            }])
            .unwrap();
            let mut game: Game = serde_json::from_value(save).unwrap();
            let (gold, arrows, sack) = (game.player().gold, game.player().arrows, game.player().sack.len());
            let mut input = CommandLine::default();
            let town = game.level().town_at(stairs).unwrap();
            for (i, (punctuation, level)) in [
                (">", isle_core::LevelId::Deep(town, 1)),
                ("<", isle_core::LevelId::Surface),
                (",", isle_core::LevelId::Surface),
            ]
            .into_iter()
            .enumerate()
            {
                let stroke = gpui::Keystroke { key_char: Some(punctuation.into()), ..Default::default() };
                let InputAction::Command(command) = input.key(&stroke, true) else {
                    panic!("punctuation is a game shortcut on an empty line");
                };
                let mut queue = VecDeque::new();
                submit_commands(&mut game, &mut queue, &mut false, !clocked, command);
                if clocked {
                    assert_eq!(game.round_number(), i as u64, "clocked shortcuts queue rather than acting early");
                    assert_eq!(queue.len(), 1);
                    game.round(Some(queue.pop_front().unwrap().1));
                } else {
                    assert!(queue.is_empty());
                }
                assert_eq!(game.player().at, level);
                assert_eq!(game.round_number(), i as u64 + 1);
                assert!(input.is_empty());
            }
            assert_eq!(
                (game.player().gold, game.player().arrows, game.player().sack.len()),
                (gold + 9, arrows + 3, sack + 1)
            );
            assert!(game.pile_at(stairs).is_none());
            assert_eq!(input.submit(), None, "shortcuts do not become the repeated typed command");
        }
    }

    #[test]
    fn clock_mode_queues_commands_and_keeps_immediate_controls() {
        let mut game = made(7);
        let pos = game.player().pos;
        let mut queue = VecDeque::new();
        let mut paused = false;
        assert!(!submit_commands(&mut game, &mut queue, &mut paused, false, "e; e; pause; look"));
        assert_eq!((game.round_number(), queue.len(), paused), (0, 2, true));
        submit_commands(&mut game, &mut queue, &mut paused, false, "e; e; e; e");
        assert_eq!(queue.len(), QUEUE);
        let (_, command) = queue.pop_front().unwrap();
        game.round(Some(command));
        assert_eq!((game.round_number(), game.player().pos), (1, (pos.0 + 1, pos.1)));
        assert!(!submit_commands(&mut game, &mut queue, &mut paused, false, "e; e; stop; pause"));
        assert!(queue.is_empty() && !paused);
        assert!(submit_commands(&mut game, &mut queue, &mut paused, false, "quit; e"));
        assert!(queue.is_empty());
    }

    #[test]
    fn a_new_creature_interrupts_a_chain_and_discards_the_remaining_commands() {
        let mut game = encounter(7, Kind::Rat, SIGHT + 1, 20);
        assert!(game.sighted().is_empty());
        submit(&mut game, "e; e; e");
        assert_eq!(game.round_number(), 1);
        assert!(!game.sighted().is_empty());
        assert!(game.log().iter().any(|line| line.text == "You break off your plan."));
        submit(&mut game, "nock");
        assert_eq!(game.round_number(), 2, "the discarded movement does not run later");
    }

    #[test]
    fn being_hurt_interrupts_even_when_a_potion_leaves_more_health_than_before() {
        let mut checked = 0;
        for seed in 1..=32 {
            let mut game = encounter(seed, Kind::Rat, 1, 5);
            let mark = game.log_total();
            let mut first = game.clone();
            first.round(Some(Command::Quaff));
            let news = (first.log_total() - mark) as usize;
            if first.log().iter().rev().take(news).any(|line| line.tone == Tone::Hurt) {
                checked += 1;
                submit(&mut game, "quaff; e");
                assert_eq!(game.round_number(), 1);
                assert_eq!(game.player(), first.player());
                assert!(game.player().hp >= 5, "healing masks the damage in a health comparison");
            }
        }
        assert!(checked > 0, "exercise actual damage, not only missed attacks");
    }

    #[test]
    fn death_interrupts_a_chain_despite_restored_health_at_the_temple() {
        let mut checked = 0;
        for seed in 1..=32 {
            let mut game = encounter(seed, Kind::Troll, 1, 1);
            let mut first = game.clone();
            first.round(Some(Command::Rest));
            if first.player().deaths > 0 {
                checked += 1;
                submit(&mut game, "rest; e; e");
                assert_eq!(game.round_number(), 1);
                assert_eq!(game.zone(), Zone::Temple);
                assert_eq!(game.player().hp, game.player().max_hp);
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn unseen_cells_are_blank_and_remembered_ones_dim() {
        let mut game = Game::new(3);
        game.answer("Ash");
        let me = game.player().pos;
        assert_eq!(cell(&game, me, &[]).0, "@ ");
        assert_eq!(cell(&game, (me.0 + 40, me.1 + 40), &[]).0, "  ");
        assert!(map_cell_art(&game, (me.0 + 40, me.1 + 40), &[]).is_none());
        assert_eq!(dimmed(0xff_ff_ff), 0x59_59_59);
    }

    #[test]
    fn landmarks_render_in_sight_and_memory_without_revealing_unknown_sites() {
        let game = made(7);
        let me = game.player().pos;
        let pos = (me.0 + 1, me.1);
        let mut save = serde_json::to_value(&game).unwrap();
        for (kind, object) in [
            (LandmarkKind::Cairn, Object::Cairn),
            (LandmarkKind::Camp, Object::Camp),
            (LandmarkKind::Shrine, Object::Shrine),
        ] {
            save["landmarks"] = serde_json::json!([isle_core::Landmark { pos, kind, opened: false }]);
            let visible: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&visible, pos, &[]).0, "&?");
            assert_eq!(map_cell_art(&visible, pos, &[]).unwrap().terrain.object, Some(object));
            assert!(matches!(map_cell_art(&visible, me, &[]).unwrap().actor, Some(MapActor::Player)));
            let mut memory = save.clone();
            memory["player"]["pos"] = serde_json::json!((me.0 + 40, me.1));
            let remembered: Game = serde_json::from_value(memory).unwrap();
            assert!(!remembered.sees(pos));
            assert_eq!(cell(&remembered, pos, &[]).0, "&?");
            assert!(map_cell_art(&remembered, pos, &[]).unwrap().remembered);
            assert_eq!(map_cell_art(&remembered, pos, &[]).unwrap().terrain.object, Some(object));
            save["landmarks"][0]["opened"] = serde_json::json!(true);
            let opened: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&opened, pos, &[]).0, "&.");
            save["landmarks"][0]["pos"] = serde_json::json!((me.0 + 80, me.1));
            let unknown: Game = serde_json::from_value(save.clone()).unwrap();
            assert!(map_cell_art(&unknown, (me.0 + 80, me.1), &[]).is_none());
            assert_eq!(cell(&unknown, (me.0 + 80, me.1), &[]).0, "  ");
        }
    }

    #[test]
    fn covenant_sites_have_distinct_markers_and_art() {
        let game = made(7);
        let me = game.player().pos;
        let pos = (me.0 + 1, me.1);
        let base = serde_json::to_value(&game).unwrap();
        for (kind, object, unopened, opened) in [
            (LandmarkKind::AshenForge, Object::AshenForge, "A!", "A."),
            (LandmarkKind::WellOfNames, Object::WellOfNames, "N!", "N."),
        ] {
            let mut save = base.clone();
            save["landmarks"] = serde_json::json!([isle_core::Landmark { pos, kind, opened: false }]);
            let visible: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&visible, pos, &[]).0, unopened);
            assert_eq!(map_cell_art(&visible, pos, &[]).unwrap().terrain.object, Some(object));

            save["covenant"]["trials"] = serde_json::json!([{
                "pos": pos, "kind": kind, "phase": "Cleared", "guards": [null, null]
            }]);
            let cleared: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&cleared, pos, &[]).0, if kind == LandmarkKind::AshenForge { "A?" } else { "N?" });

            save["landmarks"][0]["opened"] = serde_json::json!(true);
            let claimed: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&claimed, pos, &[]).0, opened);

            save["player"]["pos"] = serde_json::json!((me.0 + 40, me.1));
            let remembered: Game = serde_json::from_value(save).unwrap();
            assert!(!remembered.sees(pos));
            assert_eq!(cell(&remembered, pos, &[]).0, opened);
            assert!(map_cell_art(&remembered, pos, &[]).unwrap().remembered);
            assert_eq!(map_cell_art(&remembered, pos, &[]).unwrap().terrain.object, Some(object));
        }
    }

    #[test]
    fn demigod_indicator_requires_the_full_equipped_set() {
        let game = made(7);
        let mut save = serde_json::to_value(&game).unwrap();
        save["player"]["weapon"] = serde_json::json!("Oathfang");
        save["player"]["armor"] = serde_json::json!("AshenCarapace");
        save["player"]["relic"] = serde_json::json!("ExileHeart");
        let complete: Game = serde_json::from_value(save.clone()).unwrap();
        assert_eq!(demigod_status(complete.player()), Some("DEMIGOD · EXILE SET"));

        for (field, value) in [
            ("weapon", serde_json::json!("ShortSword")),
            ("armor", serde_json::Value::Null),
            ("relic", serde_json::Value::Null),
        ] {
            let mut incomplete = save.clone();
            incomplete["player"][field] = value;
            let incomplete: Game = serde_json::from_value(incomplete).unwrap();
            assert_eq!(demigod_status(incomplete.player()), None, "{field}");
        }
    }

    #[test]
    fn herb_patches_render_only_when_known_and_unpicked_with_ordinary_actor_priority() {
        let game = made(7);
        let me = game.player().pos;
        let pos = (me.0 + 1, me.1);
        let mut save = serde_json::to_value(&game).unwrap();
        for herb in [isle_core::Herb::Woundleaf, isle_core::Herb::Bogmint, isle_core::Herb::Sunroot] {
            save["herbs"] = serde_json::json!([isle_core::HerbPatch { pos, herb, picked: false }]);
            let visible: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&visible, pos, &[]).0, "h*");
            assert_eq!(map_cell_art(&visible, pos, &[]).unwrap().terrain.object, Some(Object::Herb(herb)));
            let mut memory = save.clone();
            memory["player"]["pos"] = serde_json::json!((me.0 + 40, me.1));
            let remembered: Game = serde_json::from_value(memory).unwrap();
            assert!(!remembered.sees(pos));
            assert_eq!(cell(&remembered, pos, &[]).0, "h*");
            assert!(map_cell_art(&remembered, pos, &[]).unwrap().remembered);
            save["player"]["pos"] = serde_json::json!(pos);
            let standing: Game = serde_json::from_value(save.clone()).unwrap();
            assert_eq!(cell(&standing, pos, &[]).0, "@ ");
            assert!(matches!(map_cell_art(&standing, pos, &[]).unwrap().actor, Some(MapActor::Player)));
            save["player"]["pos"] = serde_json::json!(me);
            save["herbs"][0]["picked"] = serde_json::json!(true);
            let picked: Game = serde_json::from_value(save.clone()).unwrap();
            assert_ne!(cell(&picked, pos, &[]).0, "h*");
            assert_ne!(map_cell_art(&picked, pos, &[]).unwrap().terrain.object, Some(Object::Herb(herb)));
            save["herbs"][0]["picked"] = serde_json::json!(false);
            save["herbs"][0]["pos"] = serde_json::json!((me.0 + 80, me.1));
            let unknown: Game = serde_json::from_value(save.clone()).unwrap();
            assert!(map_cell_art(&unknown, (me.0 + 80, me.1), &[]).is_none());
            assert_eq!(cell(&unknown, (me.0 + 80, me.1), &[]).0, "  ");
        }
    }

    #[test]
    fn cinder_watch_changes_its_known_map_marker_and_scenery_after_clearing_and_claiming() {
        let mut game = made(7);
        let pos = serde_json::to_value(&game).unwrap()["cinder_watch"]["pos"].clone();
        let pos: Pos = serde_json::from_value(pos).unwrap();
        assert!(map_cell_art(&game, pos, &[]).is_none());
        let mut save = serde_json::to_value(&game).unwrap();
        save["player"]["pos"] = serde_json::json!((pos.0 + 1, pos.1));
        game = serde_json::from_value(save).unwrap();
        game.instant(Instant::Look);
        assert_eq!(cell(&game, pos, &[]).0, "W!");
        assert_eq!(map_cell_art(&game, pos, &[]).unwrap().terrain.object, Some(Object::Watchtower { cleared: false }));
        let mut save = serde_json::to_value(&game).unwrap();
        save["cinder_watch"]["phase"] = serde_json::json!("Cleared");
        let cleared: Game = serde_json::from_value(save.clone()).unwrap();
        assert_eq!(cell(&cleared, pos, &[]).0, "W?");
        assert_eq!(
            map_cell_art(&cleared, pos, &[]).unwrap().terrain.object,
            Some(Object::Watchtower { cleared: true })
        );
        let index = game.landmarks().iter().position(|site| site.pos == pos).unwrap();
        save["landmarks"][index]["opened"] = serde_json::json!(true);
        let claimed: Game = serde_json::from_value(save.clone()).unwrap();
        assert_eq!(cell(&claimed, pos, &[]).0, "W.");
        save["player"]["pos"] = serde_json::json!((pos.0 + 30, pos.1));
        let memory: Game = serde_json::from_value(save).unwrap();
        assert_eq!(cell(&memory, pos, &[]).0, "W.");
        assert!(map_cell_art(&memory, pos, &[]).unwrap().remembered);
    }

    #[test]
    fn warped_neighborhood_uses_only_known_ground_and_tints() {
        let game = made(7);
        let me = game.player().pos;
        let mut unknown = 0;
        let mut known = 0;
        for y in me.1 - SIGHT..=me.1 + SIGHT {
            for x in me.0 - SIGHT..=me.0 + SIGHT {
                let Some(cell) = map_cell_art(&game, (x, y), &[]) else { continue };
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let at = (x + dx, y + dy);
                        let expected = if game.sees(at) || game.remembers(at) {
                            known += 1;
                            terrain_art(game.level().tile(at), game.level().ground(at), false, at)
                        } else {
                            unknown += 1;
                            cell.terrain
                        };
                        assert_eq!(cell.ground.around[(dy + 2) as usize][(dx + 2) as usize], expected.ground);
                        if dx.abs() <= 1 && dy.abs() <= 1 {
                            assert_eq!(cell.ground.tints[(dy + 1) as usize][(dx + 1) as usize], expected.tint);
                            assert_eq!(cell.ground.styles[(dy + 1) as usize][(dx + 1) as usize], expected.style);
                            assert_eq!(cell.ground.flavors[(dy + 1) as usize][(dx + 1) as usize], expected.flavor);
                        }
                    }
                }
            }
        }
        assert!(unknown > 0 && known > 0);
    }

    #[test]
    fn visual_layers_keep_actor_priority_and_never_reveal_remembered_contents() {
        let mut game = encounter(7, Kind::Rat, 1, 20);
        let me = game.player().pos;
        let fixed_at = (me.0 + 1, me.1);
        let item_at = (me.0 + 2, me.1);
        let labels = vec![(fixed_at, 'A')];
        assert!(matches!(map_cell_art(&game, me, &labels).unwrap().actor, Some(MapActor::Player)));
        assert!(matches!(
            map_cell_art(&game, fixed_at, &labels).unwrap().actor,
            Some(MapActor::Creature { creature, label: 'A' }) if creature.wild.is_none()
        ));

        let mut save = serde_json::to_value(&game).unwrap();
        save["piles"] = serde_json::to_value([
            isle_core::Pile { at: isle_core::LevelId::Surface, pos: fixed_at, items: vec![Item::Gold(2)] },
            isle_core::Pile { at: isle_core::LevelId::Surface, pos: item_at, items: vec![Item::Potion] },
            isle_core::Pile { at: isle_core::LevelId::Surface, pos: me, items: vec![Item::Ring] },
        ])
        .unwrap();
        game = serde_json::from_value(save.clone()).unwrap();
        assert!(matches!(map_cell_art(&game, fixed_at, &labels).unwrap().actor, Some(MapActor::Creature { .. })));
        assert!(matches!(map_cell_art(&game, item_at, &labels).unwrap().actor, Some(MapActor::Item(Object::Potion))));
        assert!(matches!(map_cell_art(&game, me, &labels).unwrap().actor, Some(MapActor::Player)));

        let clade = blah_core::clades(7)[0].clone();
        let wild = isle_core::Wildlife {
            lineage: isle_core::Lineage::Region(blah_core::SpeciesId {
                region: blah_core::World::new(7).region_at(0, 0),
                clade: clade.id,
            }),
            name: "Test beast".into(),
            genome: clade.base,
        };
        save["creatures"][0]["kind"] = serde_json::json!(Kind::Wild);
        save["creatures"][0]["wild"] = serde_json::to_value(wild).unwrap();
        game = serde_json::from_value(save.clone()).unwrap();
        assert!(matches!(
            map_cell_art(&game, fixed_at, &[(fixed_at, 'A')]).unwrap().actor,
            Some(MapActor::Creature { creature, .. }) if creature.wild.is_some()
        ));

        let remembered_at = item_at;
        assert!(game.remembers(remembered_at));
        save["player"]["pos"] = serde_json::json!((me.0 + 40, me.1 + 40));
        game = serde_json::from_value(save).unwrap();
        assert!(!game.sees(remembered_at));
        let remembered = map_cell_art(&game, remembered_at, &[]).unwrap();
        assert!(remembered.remembered);
        assert!(remembered.actor.is_none(), "remembered terrain must not ghost creatures or items");
    }

    #[test]
    fn log_lines_wrap_at_spaces_and_split_long_words() {
        assert_eq!(wrap("a bb ccc", 4), vec!["a bb", "ccc"]);
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("", 4), vec![""]);
        let shop = isle_core::SHOP.iter().map(|(i, p)| format!("{} {p}", i.name())).collect::<Vec<_>>().join(", ");
        assert!(wrap(&shop, LOG_CHARS - 2).iter().all(|row| row.chars().count() <= LOG_CHARS - 2));
        const { assert!(LOG_CHARS > 80) };
    }

    #[test]
    fn sprites_follow_sight_and_do_not_replace_the_player_or_authored_creatures() {
        let mut game = encounter(7, Kind::Rat, 1, 20);
        let me = game.player().pos;
        let at = (me.0 + 1, me.1);
        assert!(sprite_creature(&game, at).is_none(), "fixed-table creatures keep their glyphs");
        let clade = blah_core::clades(7)[0].clone();
        let wild = isle_core::Wildlife {
            lineage: isle_core::Lineage::Region(blah_core::SpeciesId {
                region: blah_core::World::new(7).region_at(0, 0),
                clade: clade.id,
            }),
            name: "Test beast".into(),
            genome: clade.base,
        };
        let mut save = serde_json::to_value(&game).unwrap();
        save["creatures"][0]["kind"] = serde_json::json!(Kind::Wild);
        save["creatures"][0]["wild"] = serde_json::to_value(wild).unwrap();
        game = serde_json::from_value(save.clone()).unwrap();
        assert!(sprite_creature(&game, at).is_some());
        assert_eq!(game.sighted()[0].label, 'A');
        assert_eq!(cell(&game, at, &[(at, 'A')]).0, "A*", "text inspection keeps the same target letter");
        save["creatures"][0]["pos"] = serde_json::json!(me);
        game = serde_json::from_value(save.clone()).unwrap();
        assert!(sprite_creature(&game, me).is_none());
        let far = (me.0 + SIGHT + 1, me.1);
        save["creatures"][0]["pos"] = serde_json::json!(far);
        game = serde_json::from_value(save).unwrap();
        assert!(sprite_creature(&game, far).is_none());
    }

    fn cave_encounter(covering: f32) -> Game {
        let mut save = serde_json::to_value(made(7)).unwrap();
        save["player"]["pos"] = serde_json::json!(made(7).level().stairs_down.unwrap());
        let mut game: Game = serde_json::from_value(save).unwrap();
        game.round(Some(Command::Down));
        let level = game.level();
        let at = (1..31)
            .flat_map(|y| (1..62).map(move |x| (x, y)))
            .find(|&(x, y)| [(x, y), (x + 1, y)].into_iter().all(|p| level.tile(p).walkable() && !level.safe(p)))
            .unwrap();
        let isle_core::LevelId::Deep(town, depth) = game.player().at else { unreachable!() };
        let clade = blah_core::clades(7)[0].clone();
        let mut genome = clade.base;
        genome.set(blah_core::Gene::Covering, covering);
        genome.set(blah_core::Gene::Cold, 0.8);
        genome.set(blah_core::Gene::Limbs, 0.9);
        let wild = isle_core::Wildlife {
            lineage: isle_core::Lineage::Deep { town, depth, clade: clade.id },
            name: "Test cave creature".into(),
            genome,
        };
        let mut save = serde_json::to_value(&game).unwrap();
        let mut creature = serde_json::to_value(encounter(7, Kind::Rat, 1, 20)).unwrap()["creatures"][0].clone();
        creature["kind"] = serde_json::json!(Kind::Wild);
        creature["wild"] = serde_json::to_value(wild).unwrap();
        creature["at"] = save["player"]["at"].clone();
        creature["pos"] = serde_json::json!(at);
        creature["home"] = creature["pos"].clone();
        save["creatures"] = serde_json::json!([creature]);
        save["player"]["pos"] = serde_json::json!((at.0 + 1, at.1));
        serde_json::from_value(save).unwrap()
    }

    #[test]
    fn cave_cues_explain_traits_and_light_matches_visible_cells_and_player_status() {
        let glowing = cave_encounter(0.95);
        let c = &glowing.creatures()[0];
        assert!(detail_sight_lines(glowing.player(), c).iter().any(|l| l.contains("luminous · spoils nearby hiding")));
        assert_eq!(states_text(&glowing).as_deref(), Some("lit · no hiding"));
        assert_eq!(cell(&glowing, glowing.player().pos, &[]).2, Some(CAVE_LIGHT));
        assert_eq!(cell(&glowing, c.pos, &[(c.pos, 'A')]).2, Some(CAVE_LIGHT));
        assert!(map_cell_art(&glowing, c.pos, &[(c.pos, 'A')]).unwrap().lit);
        let listener = cave_encounter(0.5);
        let c = &listener.creatures()[0];
        assert!(detail_sight_lines(listener.player(), c).iter().any(|l| l.contains("tremorsense · quiet steps help")));
        assert_eq!(states_text(&listener), None);
        assert_eq!(cell(&listener, listener.player().pos, &[]).2, None);
        assert!(!map_cell_art(&listener, c.pos, &[(c.pos, 'A')]).unwrap().lit);
        for game in [&glowing, &listener] {
            for line in detail_sight_lines(game.player(), &game.creatures()[0]) {
                assert!(line.chars().count() <= 40, "trait cues fit the panel at 12pt");
            }
        }
    }

    #[test]
    fn the_window_fits_a_laptop_screen() {
        assert!(f32::from(WINDOW_WIDTH) <= 1000.0 && f32::from(WINDOW_HEIGHT) <= 860.0);
    }

    #[test]
    fn every_practice_fits_the_inspection_panel() {
        let game = made(7);
        let lines = skill_lines(game.player());
        assert_eq!(lines.len(), Practice::ALL.len());
        for practice in Practice::ALL {
            assert!(lines.contains(&format!("{}: novice", practice.name())));
        }
        assert!((lines.len() as f32 + 4.0) * LINE + 11.0 + 2.0 * INSET < MAP_H);
    }
}
