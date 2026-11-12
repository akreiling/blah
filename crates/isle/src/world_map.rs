//! The remembered surface map and its explorer controls.

use std::collections::BTreeSet;

use gpui::{Div, Keystroke, div, prelude::*, px, rgb};
use isle_core::{Game, Pos, SurfaceMapCell, Tile, Town, Zone};

const SCREEN: u32 = 0x07_08_0b;
const TEXT: u32 = 0xd8_d8_c8;
const DIM: u32 = 0x76_79_88;
const HEADING: u32 = 0xf0_c0_4a;
const YOU: u32 = 0xff_f2_7a;
const CELL_SIZES: [f32; 4] = [8.0, 12.0, 18.0, 26.0];
const CELL_STEPS: [i32; 4] = [8, 4, 2, 1];
const MAP_WIDTH: f32 = 740.0;
const MAP_HEIGHT: f32 = 624.0;

/// The map cursor and zoom level.
pub struct WorldMap {
    center: Pos,
    zoom: usize,
}

struct MapFrame {
    origin: Pos,
    columns: i32,
    rows: i32,
    step: i32,
    cells: Vec<Option<SurfaceMapCell>>,
}

impl WorldMap {
    pub fn new(game: &Game) -> Self {
        Self { center: game.surface_map_player_pos(), zoom: 1 }
    }

    /// Handle a map key. Returns true when the map should close.
    pub fn key(&mut self, stroke: &Keystroke, game: &Game) -> bool {
        let modifiers = stroke.modifiers;
        if modifiers.alt || modifiers.control || modifiers.platform {
            return false;
        }
        let key = stroke.key.as_str();
        if key == "escape" || key.eq_ignore_ascii_case("m") {
            return true;
        }
        if matches!(key, "home" | "f" | "c") {
            self.center = game.surface_map_player_pos();
            return false;
        }
        let pan = CELL_STEPS[self.zoom] * if modifiers.shift { 8 } else { 1 };
        let (dx, dy) = match key {
            "up" | "k" => (0, -pan),
            "down" | "j" => (0, pan),
            "left" | "h" => (-pan, 0),
            "right" | "l" => (pan, 0),
            _ => (0, 0),
        };
        if dx != 0 || dy != 0 {
            self.center = (self.center.0.saturating_add(dx), self.center.1.saturating_add(dy));
            return false;
        }
        let typed = stroke.key_char.as_deref().unwrap_or(key);
        match typed {
            "+" | "=" => self.zoom = (self.zoom + 1).min(CELL_SIZES.len() - 1),
            "-" | "_" => self.zoom = self.zoom.saturating_sub(1),
            "0" => self.zoom = 1,
            _ => {}
        }
        false
    }

    fn frame(&self, game: &Game, debug_reveal: bool) -> MapFrame {
        let size = CELL_SIZES[self.zoom];
        let columns = (MAP_WIDTH / size) as i32;
        let rows = (MAP_HEIGHT / size) as i32;
        let step = CELL_STEPS[self.zoom];
        let origin = (self.center.0.saturating_sub(columns / 2 * step), self.center.1.saturating_sub(rows / 2 * step));
        let cells = game.surface_map_samples(origin, columns, rows, step, debug_reveal);
        MapFrame { origin, columns, rows, step, cells }
    }

    /// The same map frame the window paints, for opt-in control inspection.
    pub fn screen(&self, game: &Game, debug_reveal: bool) -> serde_json::Value {
        let frame = self.frame(game, debug_reveal);
        let player = game.surface_map_player_pos();
        let towns: BTreeSet<_> = frame.cells.iter().flatten().filter_map(|c| c.settlement).collect();
        let rows: Vec<String> = (0..frame.rows)
            .map(|y| {
                (0..frame.columns)
                    .map(|x| {
                        if self.player_cell(&frame, player) == (x, y) {
                            return '@';
                        }
                        frame.cells[(y * frame.columns + x) as usize]
                            .map_or(' ', |cell| map_cell(cell, frame.step, debug_reveal).0)
                    })
                    .collect()
            })
            .collect();
        serde_json::json!({
            "mode": "world_map", "view": rows, "center": self.center,
            "origin": frame.origin, "step": frame.step, "zoom": self.zoom,
            "debug_reveal": debug_reveal,
            "known_cells": frame.cells.iter().flatten().count(),
            "settlements": towns.into_iter().map(|t| t.name()).collect::<Vec<_>>(),
        })
    }

    fn player_cell(&self, frame: &MapFrame, player: Pos) -> Pos {
        (
            ((i64::from(player.0) - i64::from(frame.origin.0)).div_euclid(i64::from(frame.step))) as i32,
            ((i64::from(player.1) - i64::from(frame.origin.1)).div_euclid(i64::from(frame.step))) as i32,
        )
    }

    pub fn render(&self, game: &Game, debug_reveal: bool) -> Div {
        let size = CELL_SIZES[self.zoom];
        let frame = self.frame(game, debug_reveal);
        let MapFrame { columns, rows, cells, .. } = &frame;
        let player = game.surface_map_player_pos();
        let mut towns = BTreeSet::<Town>::new();
        for cell in cells.iter().flatten() {
            if let Some(town) = cell.settlement {
                towns.insert(town);
            }
        }
        let grid = div().flex().flex_col().children((0..*rows).map(|y| {
            let row = (0..*columns).map(|x| {
                let cell = cells[(y * columns + x) as usize];
                let (glyph, foreground, background) = if self.player_cell(&frame, player) == (x, y) {
                    ('@', YOU, 0x27_25_14)
                } else {
                    cell.map_or((' ', DIM, SCREEN), |cell| map_cell(cell, frame.step, debug_reveal))
                };
                div()
                    .w(px(size))
                    .h(px(size))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(background))
                    .text_color(rgb(foreground))
                    .text_size(px((size * 0.85).max(7.0)))
                    .line_height(px(size))
                    .child(glyph.to_string())
            });
            div().flex().flex_row().h(px(size)).children(row)
        }));
        let settlement_names: Vec<String> = towns.iter().take(5).map(|town| town.name()).collect();
        let settlements = if settlement_names.is_empty() {
            "No settlements discovered in this view".to_owned()
        } else {
            let more = towns.len().saturating_sub(settlement_names.len());
            format!("{}{}", settlement_names.join(" · "), if more > 0 { format!(" · +{more}") } else { String::new() })
        };
        let mode = if debug_reveal { "DEBUG REVEAL · ALL TERRAIN" } else { "ISLE MAP · SEEN TERRAIN ONLY" };
        div()
            .size_full()
            .bg(rgb(SCREEN))
            .font_family("Menlo")
            .text_size(px(12.0))
            .text_color(rgb(TEXT))
            .px(px(12.0))
            .py(px(8.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_color(rgb(HEADING))
                    .child(mode)
                    .child(format!(
                        "center {:+}, {:+} · {} cells/glyph · zoom {}/{}",
                        self.center.0,
                        self.center.1,
                        frame.step,
                        self.zoom + 1,
                        CELL_SIZES.len()
                    )),
            )
            .child(div().h(px(4.0)))
            .child(div().flex_1().min_h_0().flex().items_center().justify_center().overflow_hidden().child(grid))
            .child(
                div()
                    .h(px(22.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .text_color(rgb(if debug_reveal { HEADING } else { TEXT }))
                    .child(if debug_reveal {
                        format!("Revealed settlements: {settlements}")
                    } else {
                        format!("Known settlements: {settlements}")
                    }),
            )
            .child(
                div()
                    .h(px(20.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .text_color(rgb(DIM))
                    .child("o settlement · M / Esc close · arrows pan · +/- zoom · F / Home center"),
            )
    }
}

fn map_cell(cell: SurfaceMapCell, step: i32, debug_reveal: bool) -> (char, u32, u32) {
    let (glyph, color) = if step > 1 && cell.settlement.is_some() {
        ('o', HEADING)
    } else {
        match (cell.tile, cell.zone) {
            (Tile::Water, _) => ('~', 0x55_91_d8),
            (Tile::Rock, _) => ('#', 0x9a_a1_ad),
            (Tile::Tree, _) => ('T', 0x59_b5_63),
            (Tile::Wall, _) => ('#', 0xc0_a6_88),
            (Tile::Floor, Zone::Shop) => ('s', 0xe0_b0_60),
            (Tile::Floor, Zone::Temple) => ('t', 0xc8_a0_e8),
            (Tile::Floor, _) => ('.', 0xa2_9b_89),
            (Tile::Grass, _) => (',', 0x70_a0_56),
            (Tile::Door, _) => ('+', 0xe0_b0_60),
            (Tile::StairsDown, _) => ('>', 0xff_e0_80),
            (Tile::StairsUp, _) => ('<', 0xff_e0_80),
            (Tile::Counter, _) => ('=', 0xe0_b0_60),
            (Tile::Altar, _) => ('*', 0xc8_a0_e8),
            (Tile::Wild(biome), _) => {
                let [r, g, b] = biome.biome().color;
                (biome.biome().glyph, u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
            }
        }
    };
    let lit = cell.visible || debug_reveal;
    let brightness = if lit { 1.0 } else { 0.72 };
    let foreground = scale_color(color, brightness);
    let background = scale_color(color, if lit { 0.30 } else { 0.18 });
    (glyph, foreground, background)
}

fn scale_color(color: u32, factor: f32) -> u32 {
    let scale = |shift: u32| ((((color >> shift) & 0xff) as f32 * factor) as u32) << shift;
    scale(16) | scale(8) | scale(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_controls_pan_zoom_recenter_and_close_without_game_commands() {
        let game = Game::new(7);
        let player = game.surface_map_player_pos();
        let mut map = WorldMap::new(&game);
        assert_eq!(map.center, player);

        map.key(&Keystroke::parse("right").unwrap(), &game);
        assert_eq!(map.center, (player.0 + 4, player.1));
        map.key(&Keystroke::parse("shift-down").unwrap(), &game);
        assert_eq!(map.center, (player.0 + 4, player.1 + 32));

        map.key(&Keystroke::parse("=").unwrap(), &game);
        assert_eq!(map.zoom, 2);
        map.key(&Keystroke::parse("-").unwrap(), &game);
        assert_eq!(map.zoom, 1);
        map.key(&Keystroke::parse("f").unwrap(), &game);
        assert_eq!(map.center, player);
        assert!(map.key(&Keystroke::parse("escape").unwrap(), &game));
    }
}
