//! The game window: map viewport on the left, status panel on the right, message log along the bottom.
//!
//! Everything is drawn from the [`Game`] on each render. The view holds no state of its own beyond focus.

use gpui::{
    App, Context, Div, FocusHandle, Focusable, FontWeight, Pixels, Rgba, SharedString, Window, div, prelude::*, px, rgb,
};

use blah_core::{Dir, Game, Tile};

use crate::{MoveEast, MoveNorth, MoveSouth, MoveWest};

/// Tiles along each side of the viewport. Odd, so the wanderer sits on the center tile.
const VIEW: i64 = 17;
/// Side of one tile, in pixels.
const TILE: f32 = 36.0;
/// Glyph size within a tile.
const GLYPH: f32 = 22.0;

/// Space around and between the frames.
const GAP: f32 = 12.0;
/// Frame border width.
const BORDER: f32 = 2.0;
/// Width of the status panel, frame included.
const PANEL: f32 = 300.0;
/// Log lines shown, newest last.
const LOG_LINES: usize = 5;
/// Height of one line of panel and log text.
const LINE: f32 = 22.0;
/// Padding inside the panel and log frames.
const INSET: f32 = 12.0;

const MAP: f32 = VIEW as f32 * TILE + 2.0 * BORDER;
const LOG: f32 = LOG_LINES as f32 * LINE + 2.0 * (INSET + BORDER);

/// Window content width: map, panel, and the gaps around them.
pub const WINDOW_WIDTH: Pixels = px(GAP + MAP + GAP + PANEL + GAP);
/// Window content height: map, log, and the gaps around them.
pub const WINDOW_HEIGHT: Pixels = px(GAP + MAP + GAP + LOG + GAP);

const FONT: &str = "Menlo";
const TEXT_SIZE: f32 = 15.0;

/// The palette: a near-black screen, dim periwinkle window frames, amber headings.
const SCREEN: u32 = 0x07_08_0b;
const FRAME: u32 = 0x55_5a_88;
const TEXT: u32 = 0xd8_d8_c8;
const DIM: u32 = 0x76_79_88;
const FAINT: u32 = 0x45_47_52;
const HEADING: u32 = 0xf0_c0_4a;
const WANDERER: u32 = 0xff_f2_7a;
const HP: u32 = 0xd0_48_48;
const MP: u32 = 0x48_78_d0;
const EMPTY_BAR: u32 = 0x20_22_2a;

/// The one window's root view.
pub struct GameView {
    game: Game,
    focus: FocusHandle,
}

impl GameView {
    /// A view of `game`, taking keyboard focus so the movement keys reach it.
    pub fn new(game: Game, window: &mut Window, cx: &mut Context<GameView>) -> GameView {
        let focus = cx.focus_handle();
        window.focus(&focus);
        GameView { game, focus }
    }

    fn step(&mut self, dir: Dir, cx: &mut Context<GameView>) {
        self.game.step(dir);
        cx.notify();
    }

    fn map(&self) -> Div {
        let (px0, py0) = self.game.position();
        let half = VIEW / 2;
        let world = self.game.world();
        let rows = (-half..=half).map(|dy| {
            let cells = (-half..=half).map(|dx| {
                let tile = world.tile(px0 + dx, py0 + dy);
                cell(&tile, dx == 0 && dy == 0)
            });
            div().flex().flex_row().children(cells)
        });
        frame().size(px(MAP)).flex().flex_col().children(rows)
    }

    fn panel(&self) -> Div {
        let (x, y) = self.game.position();
        let tile = self.game.tile();
        let biome = tile.biome.biome();
        let seed = self.game.world().seed();

        let wanderer = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(heading_row("WANDERER", "Lv 1".into()))
            .child(bar("HP", 20, 20, HP))
            .child(bar("MP", 8, 10, MP))
            .child(pair("Turn", self.game.turn().to_string()));

        // The region's name, then the biome underfoot, which can differ at a region's ragged edges.
        let place = div()
            .flex()
            .flex_col()
            .child(heading("WHERE"))
            .child(pair("Position", format!("{x}, {y}")))
            .child(div().h(px(LINE)).truncate().text_color(rgb(TEXT)).child(capitalized(&self.game.region().name)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(LINE))
                    .child(div().size(px(12.0)).bg(color(biome.color)).border_1().border_color(rgb(FRAME)))
                    .child(div().text_color(rgb(DIM)).child(biome.name)),
            );

        let debug = div()
            .flex()
            .flex_col()
            .text_color(rgb(DIM))
            .child(heading("DEBUG"))
            .child(field("elevation", tile.elevation))
            .child(field("moisture", tile.moisture))
            .child(field("temperature", tile.temperature))
            .child(field("age", tile.age))
            .child(pair_dim("seed", seed.to_string()));

        frame()
            .w(px(PANEL))
            .h(px(MAP))
            .p(px(INSET))
            .flex()
            .flex_col()
            .child(wanderer)
            .child(rule())
            .child(place)
            .child(rule())
            .child(debug)
            .child(div().flex_grow())
            .child(div().text_color(rgb(FAINT)).text_size(px(12.0)).child("arrows · hjkl · wasd    q quit"))
    }

    fn log(&self) -> Div {
        let lines: Vec<&str> = self.game.log().last(LOG_LINES).collect();
        let newest = lines.len().saturating_sub(1);
        let rows = lines.into_iter().enumerate().map(|(i, line)| {
            let shade = if i == newest { TEXT } else { DIM };
            div()
                .h(px(LINE))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_color(rgb(shade))
                .child(SharedString::from(format!("› {line}")))
        });
        frame().w_full().h(px(LOG)).p(px(INSET)).flex().flex_col().justify_end().children(rows)
    }
}

impl Focusable for GameView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GameView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<GameView>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &MoveNorth, _, cx| this.step(Dir::North, cx)))
            .on_action(cx.listener(|this, _: &MoveSouth, _, cx| this.step(Dir::South, cx)))
            .on_action(cx.listener(|this, _: &MoveEast, _, cx| this.step(Dir::East, cx)))
            .on_action(cx.listener(|this, _: &MoveWest, _, cx| this.step(Dir::West, cx)))
            .size_full()
            .bg(rgb(SCREEN))
            .font_family(FONT)
            .text_size(px(TEXT_SIZE))
            .line_height(px(LINE))
            .text_color(rgb(TEXT))
            .p(px(GAP))
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(div().flex().flex_row().gap(px(GAP)).child(self.map()).child(self.panel()))
            .child(self.log())
    }
}

/// One map tile: the biome's color with its glyph, or the wanderer.
fn cell(tile: &Tile, wanderer: bool) -> Div {
    let biome = tile.biome.biome();
    let (bg, fg, glyph, weight) = if wanderer {
        (color(scale(biome.color, 0.3)), rgb(WANDERER), '@', FontWeight::BOLD)
    } else {
        (color(biome.color), color(glyph_shade(biome.color)), biome.glyph, FontWeight::NORMAL)
    };
    div()
        .size(px(TILE))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .text_color(fg)
        .text_size(px(GLYPH))
        .line_height(px(TILE))
        .font_weight(weight)
        .child(SharedString::from(glyph.to_string()))
}

/// A bordered window, the retro kind.
fn frame() -> Div {
    div().flex_none().bg(rgb(SCREEN)).border(px(BORDER)).border_color(rgb(FRAME))
}

fn heading(text: &'static str) -> Div {
    div().h(px(LINE)).text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(text)
}

fn heading_row(text: &'static str, right: String) -> Div {
    div().flex().flex_row().justify_between().child(heading(text)).child(div().text_color(rgb(TEXT)).child(right))
}

fn rule() -> Div {
    div().my(px(10.0)).h(px(1.0)).w_full().bg(rgb(FAINT))
}

fn pair(label: &'static str, value: String) -> Div {
    div()
        .h(px(LINE))
        .flex()
        .flex_row()
        .justify_between()
        .child(div().text_color(rgb(DIM)).child(label))
        .child(div().text_color(rgb(TEXT)).child(value))
}

fn pair_dim(label: &'static str, value: String) -> Div {
    div().h(px(LINE)).flex().flex_row().justify_between().child(label).child(value)
}

fn field(label: &'static str, value: f32) -> Div {
    pair_dim(label, format!("{value:.3}"))
}

/// A segmented gauge: label, ten cells filled in proportion, and the number.
fn bar(label: &'static str, value: u32, max: u32, fill: u32) -> Div {
    const CELLS: u32 = 10;
    let filled = (value * CELLS).div_ceil(max.max(1));
    let cells = (0..CELLS).map(|i| div().w(px(14.0)).h(px(12.0)).bg(rgb(if i < filled { fill } else { EMPTY_BAR })));
    div()
        .h(px(LINE))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(div().w(px(24.0)).text_color(rgb(DIM)).child(label))
        .child(div().flex().flex_row().gap(px(2.0)).children(cells))
        .child(div().flex_grow())
        .child(div().text_color(rgb(TEXT)).child(value.to_string()))
}

/// A name as a heading: "the Glass Marsh" becomes "The Glass Marsh".
fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

fn color([r, g, b]: [u8; 3]) -> Rgba {
    rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
}

fn scale([r, g, b]: [u8; 3], k: f32) -> [u8; 3] {
    let s = |c: u8| (f32::from(c) * k).round() as u8;
    [s(r), s(g), s(b)]
}

/// A shade of the tile's own color that stands out on it: darker on light tiles, lighter on dark ones.
fn glyph_shade(c @ [r, g, b]: [u8; 3]) -> [u8; 3] {
    let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
    if luma > 110.0 {
        scale(c, 0.45)
    } else {
        let lift = |v: u8| (f32::from(v) + (255.0 - f32::from(v)) * 0.45).round() as u8;
        [lift(r), lift(g), lift(b)]
    }
}
