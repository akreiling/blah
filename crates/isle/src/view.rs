//! The window: the map on the left drawn two characters to a cell, the character panel on the right, the
//! log beneath, and the command line at the bottom. Commands resolve rounds, unless a clock is requested.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant as Clock};

use gpui::{
    App, Context, Div, FocusHandle, Focusable, FontWeight, KeyDownEvent, Pixels, SharedString, Task, Window, div,
    prelude::*, px, rgb,
};

use isle_core::{Command, Game, Item, Kind, Parsed, Phase, Pos, SIGHT, Tile, Tone, Zone, distance};

/// Cells along each side of the map.
const VIEW: i32 = 2 * SIGHT + 1;
const CELL_W: f32 = 40.0;
const CELL_H: f32 = 36.0;
const GLYPH: f32 = 22.0;
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
const MP: u32 = 0x5c_8c_f0;
const CLOCK: u32 = 0x8c_c0_6a;
const EMPTY_BAR: u32 = 0x20_22_2a;

/// The one window's view.
pub struct IsleView {
    game: Game,
    path: PathBuf,
    save_error: Option<String>,
    focus: FocusHandle,
    input: String,
    queue: VecDeque<(String, Command)>,
    last_line: String,
    /// The round clock; without one, each command is a round and the world waits.
    round: Option<Duration>,
    started: Clock,
    paused: bool,
    /// Log total before the latest round: lines past it are news.
    mark: u64,
    _ticker: Option<Task<()>>,
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
        let mark = game.log_total().saturating_sub(1);
        IsleView {
            game,
            path,
            save_error: None,
            focus,
            input: String::new(),
            queue: VecDeque::new(),
            last_line: String::new(),
            round,
            started: Clock::now(),
            paused: false,
            mark,
            _ticker: ticker,
        }
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

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<IsleView>) {
        let stroke = &event.keystroke;
        if stroke.modifiers.platform || stroke.modifiers.control {
            return;
        }
        match stroke.key.as_str() {
            "enter" => self.submit(cx),
            "backspace" => _ = self.input.pop(),
            "escape" => self.input.clear(),
            "up" => self.input = self.last_line.clone(),
            _ => {
                if let Some(typed) = &stroke.key_char
                    && typed.chars().all(|c| !c.is_control())
                    && self.input.chars().count() < 64
                {
                    self.input.push_str(typed);
                }
            }
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<IsleView>) {
        let typed = std::mem::take(&mut self.input).trim().to_owned();
        let line = if typed.is_empty() { self.last_line.clone() } else { typed };
        if line.is_empty() {
            return;
        }
        self.last_line = line.clone();
        if self.game.phase() != &Phase::Play {
            self.mark = self.game.log_total();
            self.game.answer(&line);
            self.started = Clock::now();
            self.save();
            return;
        }
        let turn_based = self.round.is_none();
        if turn_based {
            self.mark = self.game.log_total();
        }
        let quit = submit_commands(&mut self.game, &mut self.queue, &mut self.paused, turn_based, &line);
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
    let mark = game.log_total();
    let before: Vec<u32> = game.sighted().iter().map(|s| s.creature.id).collect();
    game.round(Some(command));
    let news = (game.log_total() - mark) as usize;
    let hurt = game.log().iter().rev().take(news).any(|line| matches!(line.tone, Tone::Hurt | Tone::Death));
    let newcomer = game.sighted().iter().any(|s| !before.contains(&s.creature.id));
    !hurt && !newcomer
}

impl Focusable for IsleView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for IsleView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<IsleView>) -> impl IntoElement {
        let title = self.save_error.clone().unwrap_or_else(|| {
            let file = self.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            format!("Isle · seed {} · {file}", self.game.seed())
        });
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.key(event, cx)))
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
            .child(div().flex().flex_row().gap(px(GAP)).child(map(&self.game)).child(self.panel()))
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
        if game.phase() != &Phase::Play {
            return panel
                .child(heading("A NEW ARRIVAL"))
                .child(dim_line("Type your answer below and press Enter."))
                .child(rule())
                .child(dim_line("Fighter: the most health; sword and armor."))
                .child(dim_line("Thief: hides, strikes from hiding,"))
                .child(dim_line("  starts with a shortbow."))
                .child(dim_line("Thaumaturge: warrior-priest; heal, stun."))
                .child(dim_line("Wizard: spark and shield; no metal armor."));
        }
        panel = panel
            .child(heading_owned(format!("{} the {}", p.name, p.class.name())))
            .child(pair(
                "Level",
                match p.next_level() {
                    Some(next) => format!("{} · {}/{next} xp", p.level, p.xp),
                    None => format!("{} · {} xp", p.level, p.xp),
                },
            ))
            .child(bar("HP", p.hp, p.max_hp, HP));
        if p.class.casts() {
            panel = panel.child(bar("MP", p.mp, p.max_mp, MP));
        }
        let bow = match p.bow {
            Some(bow) => format!("{} · {} arrows{}", bow.name(), p.arrows, if p.nocked { " · NOCKED" } else { "" }),
            None => "none".into(),
        };
        panel = panel
            .child(pair("Armor class", format!("{}", p.ac())))
            .child(pair("Gold", format!("{}", p.gold)))
            .child(rule())
            .child(pair("Hand", p.weapon.name().into()))
            .child(pair("Bow", bow))
            .child(pair(
                "Worn",
                match (p.armor, p.ring) {
                    (Some(a), true) => format!("{} · ring", a.name()),
                    (Some(a), false) => a.name().into(),
                    (None, true) => "ring of warding".into(),
                    (None, false) => "nothing".into(),
                },
            ));
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
        if !states.is_empty() {
            panel = panel.child(div().h(px(LINE)).text_color(rgb(HEADING)).child(states.join(" · ")));
        }
        let place = match game.zone() {
            Zone::Shop => format!("{} · shop", game.level().name),
            Zone::Temple => format!("{} · temple", game.level().name),
            Zone::Open => game.level().name.to_owned(),
        };
        panel = panel.child(rule()).child(pair("Place", place));
        let Some(round) = self.round else {
            return self.sighted(panel.child(pair("Round", format!("{}", game.round_number() + 1))));
        };
        let left = round.as_secs_f32() * (1.0 - self.clock());
        let clock = if self.paused { "PAUSED".to_owned() } else { format!("{left:.0}s") };
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
        let sighted = game.sighted();
        if sighted.is_empty() {
            panel = panel.child(dim_line("Nothing stirs."));
        }
        for s in sighted.iter().take(6) {
            let c = s.creature;
            let stats = c.kind.stats();
            let (dx, dy) = (c.pos.0 - p.pos.0, c.pos.1 - p.pos.1);
            let way = isle_core::Dir::from_delta(dx, dy).map_or("", isle_core::Dir::short);
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
            panel = panel.child(
                div()
                    .h(px(LINE))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .whitespace_nowrap()
                    .child(
                        div().w(px(14.0)).text_color(rgb(creature_color(c.kind, c.hunting))).child(s.label.to_string()),
                    )
                    .child(div().text_color(rgb(TEXT)).child(stats.name))
                    .child(div().text_color(rgb(DIM)).child(notes.join(" · "))),
            );
        }
        panel
    }

    fn prompt(&self) -> Div {
        let creating = self.game.phase() != &Phase::Play;
        let hint = if creating { "your answer" } else { "type a command (help lists them)" };
        let cursor = div().w(px(9.0)).h(px(18.0)).bg(rgb(HEADING));
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
            .when(!self.input.is_empty(), |d| {
                d.child(div().text_color(rgb(TEXT)).child(SharedString::from(self.input.clone())))
            })
            .child(cursor)
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
        Zone::Open => None,
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
    };
    (glyph.into(), if visible { color } else { dimmed(color) }, back)
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

fn map(game: &Game) -> Div {
    let me = game.player().pos;
    let labels: Vec<(Pos, char)> = game.sighted().iter().map(|s| (s.creature.pos, s.label)).collect();
    let rows = (-SIGHT..=SIGHT).map(|dy| {
        let cells = (-SIGHT..=SIGHT).map(|dx| {
            let (glyph, color, back) = cell(game, (me.0 + dx, me.1 + dy), &labels);
            div()
                .w(px(CELL_W))
                .h(px(CELL_H))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(GLYPH))
                .text_color(rgb(color))
                .when_some(back, |d, b| d.bg(rgb(b)))
                .when(dx == 0 && dy == 0, |d| d.font_weight(FontWeight::BOLD))
                .child(glyph)
        });
        div().flex().flex_row().children(cells.collect::<Vec<_>>())
    });
    frame().w(px(MAP_W)).h(px(MAP_H)).flex().flex_col().children(rows.collect::<Vec<_>>())
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
    div().h(px(LINE)).text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(text)
}

fn heading_owned(text: String) -> Div {
    div().h(px(LINE)).text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(SharedString::from(text))
}

fn dim_line(text: &'static str) -> Div {
    div().h(px(LINE)).text_size(px(12.0)).text_color(rgb(DIM)).child(text)
}

fn rule() -> Div {
    div().my(px(5.0)).h(px(1.0)).w_full().bg(rgb(FAINT))
}

fn pair(label: &'static str, value: String) -> Div {
    div()
        .h(px(LINE))
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
        game.answer("fighter");
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
        let mut creature = save["creatures"][0].clone();
        creature["kind"] = serde_json::to_value(kind).unwrap();
        creature["at"] = serde_json::json!("Surface");
        creature["pos"] = serde_json::json!([x + offset, y]);
        creature["home"] = creature["pos"].clone();
        creature["hp"] = serde_json::json!(kind.stats().hp);
        creature["hunting"] = serde_json::json!(false);
        creature["spawn"] = serde_json::Value::Null;
        save["creatures"] = serde_json::json!([creature]);
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
        submit(&mut game, "look; inventory; help; list; nonsense; pause; stop");
        assert_eq!(game.round_number(), 2);
        assert_eq!(game.creatures(), creatures);
        submit(&mut game, "wait; wait; wait; wait; wait; wait");
        assert_eq!(game.round_number(), 7, "at most five rounds per line");
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
        game.answer("fighter");
        let me = game.player().pos;
        assert_eq!(cell(&game, me, &[]).0, "@ ");
        assert_eq!(cell(&game, (me.0 + 40, me.1 + 40), &[]).0, "  ");
        assert_eq!(dimmed(0xff_ff_ff), 0x59_59_59);
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
    fn the_window_fits_a_laptop_screen() {
        assert!(f32::from(WINDOW_WIDTH) <= 1000.0 && f32::from(WINDOW_HEIGHT) <= 860.0);
    }
}
