//! The game window: a title strip, the map viewport on the left, the status panel on the right, and the
//! message log along the bottom.
//!
//! Everything is drawn from the [`Game`] and optional workshop draft on each render. The view remembers
//! the draft until prepared or cancelled, where the log stood before the latest action, and cached sprites.

use gpui::{
    App, Context, Div, FocusHandle, Focusable, FontWeight, Pixels, Rgba, SharedString, Window, div, img, prelude::*,
    px, rgb,
};

use blah_core::{
    Aim, Comparison, Delivery, Dir, Element, Game, Gene, LogKind, MAX_AREA_RADIUS, MAX_BURST_POWER, MAX_FOCUS, Monster,
    Shape, SpeciesId, Tile, Trigger, VIEW_RADIUS, hash_point,
};

use crate::sprites::{Key, SPRITE_PX, SpriteCache};
use crate::workshop::{Edit, Workshop};
use crate::{
    CancelWorkshop, MoveEast, MoveNorth, MoveSouth, MoveWest, OpenWorkshop, PrepareWorkshop, RecoverFocus, ToggleSneak,
    Unleash, Wait, WorkshopAim, WorkshopDaggerExample, WorkshopDelivery, WorkshopElement, WorkshopPayload,
    WorkshopPowerDown, WorkshopPowerUp, WorkshopShape, WorkshopStaffExample, WorkshopTrigger, WorkshopWeapon,
};

/// Tiles along each side of the viewport. Odd, so the wanderer sits on the center tile.
const VIEW: i64 = 2 * VIEW_RADIUS + 1;
/// Side of one tile, in pixels.
const TILE: f32 = 36.0;
/// Glyph size within a tile.
const GLYPH: f32 = 22.0;

/// Height of the title strip, which sits where the system title bar would.
const TITLE: f32 = 28.0;
/// Space around and between the frames.
const GAP: f32 = 12.0;
/// Frame border width.
const BORDER: f32 = 2.0;
/// Width of the status panel, frame included.
const PANEL: f32 = 352.0;
/// Log lines shown, newest last.
const LOG_LINES: usize = 7;
/// Height of one line of panel and log text.
const LINE: f32 = 22.0;
/// Padding inside the panel and log frames.
const INSET: f32 = 12.0;
/// Species listed under NEARBY.
const NEARBY: usize = 2;
/// Trait changes listed per nearby species, strongest first.
const DELTAS: usize = 3;
/// Side of the colored card a NEARBY sprite sits on.
const CARD: f32 = 34.0;

const MAP: f32 = VIEW as f32 * TILE + 2.0 * BORDER;
const LOG: f32 = LOG_LINES as f32 * LINE + 2.0 * (INSET + BORDER);

/// Window width: map, panel, and the gaps around them.
pub const WINDOW_WIDTH: Pixels = px(GAP + MAP + GAP + PANEL + GAP);
/// Window height: title strip, map, log, and the gaps around them.
pub const WINDOW_HEIGHT: Pixels = px(TITLE + MAP + GAP + LOG + GAP);

const FONT: &str = "Menlo";
const TEXT_SIZE: f32 = 15.0;
const SMALL_SIZE: f32 = 12.0;
const SMALL_LINE: f32 = 15.0;

/// The palette: a near-black screen, dim periwinkle window frames, amber headings.
const SCREEN: u32 = 0x07_08_0b;
const FRAME: u32 = 0x55_5a_88;
const TEXT: u32 = 0xd8_d8_c8;
const DIM: u32 = 0x76_79_88;
const FAINT: u32 = 0x45_47_52;
const HEADING: u32 = 0xf0_c0_4a;
const WANDERER: u32 = 0xff_f2_7a;
const HP: u32 = 0xd0_48_48;
const EMPTY_BAR: u32 = 0x20_22_2a;
/// The wanderer's tile on the turn it was hurt.
const HURT_TILE: u32 = 0x7a_12_12;
/// A hurt monster's health pip.
const PIP: u32 = 0xe8_50_40;
const PIP_BACK: u32 = 0x10_08_08;
/// Arrows for a trait that went up, and one that went down.
const MORE: u32 = 0xf0_9a_4a;
const LESS: u32 = 0x5c_b0_e8;

/// The color of a log line of `kind`: bright if the latest action added it, dim if older. Hurt, kills,
/// death, and new regions stand out; the rest is plain text.
const fn tint(kind: LogKind, new: bool) -> u32 {
    match (kind, new) {
        (LogKind::Blocked | LogKind::Attack | LogKind::Missed, true) => TEXT,
        (LogKind::Blocked | LogKind::Attack | LogKind::Missed, false) => DIM,
        (LogKind::Hurt, true) => 0xff_6a_5a,
        (LogKind::Hurt, false) => 0x8c_44_3e,
        (LogKind::Kill, true) => 0x8c_e0_6a,
        (LogKind::Kill, false) => 0x4e_7a_40,
        (LogKind::Death, true) => 0xe0_70_ff,
        (LogKind::Death, false) => 0x7a_48_8a,
        (LogKind::Region, true) => HEADING,
        (LogKind::Region, false) => 0x86_70_3a,
    }
}

/// The one window's root view.
pub struct GameView {
    game: Game,
    focus: FocusHandle,
    /// [`Log::total`](blah_core::Log::total) before the latest action: lines past it are that action's.
    mark: u64,
    sprites: SpriteCache,
    workshop: Option<Workshop>,
}

impl GameView {
    /// A view of `game`, taking keyboard focus so the movement keys reach it. Log lines past `mark` (a
    /// [`Log::total`](blah_core::Log::total)) show as new.
    pub fn new(game: Game, mark: u64, window: &mut Window, cx: &mut Context<GameView>) -> GameView {
        let focus = cx.focus_handle();
        window.focus(&focus);
        GameView { game, focus, mark, sprites: SpriteCache::default(), workshop: None }
    }

    fn act(&mut self, action: impl FnOnce(&mut Game), cx: &mut Context<GameView>) {
        if apply_play_action(&mut self.game, &mut self.mark, self.workshop.as_ref(), action) {
            cx.notify();
        }
    }

    fn edit_workshop(&mut self, edit: Edit, cx: &mut Context<GameView>) {
        if let Some(workshop) = &mut self.workshop {
            workshop.apply(edit);
            cx.notify();
        }
    }

    fn prepare_workshop(&mut self, cx: &mut Context<GameView>) {
        let Some(workshop) = &mut self.workshop else {
            return;
        };
        self.mark = self.game.log().total();
        let result = workshop.prepare(&mut self.game);
        if result.is_ok() {
            self.workshop = None;
        }
        cx.notify();
    }

    /// The kinds of the lines the latest action added.
    fn news(&self) -> impl Iterator<Item = LogKind> {
        let log = self.game.log();
        let added = usize::try_from(log.total() - self.mark).unwrap_or(usize::MAX);
        log.entries().rev().take(added).map(|e| e.kind)
    }
}

/// Shared by every gameplay key. Draft editing must not advance the world behind the workshop.
fn apply_play_action(
    game: &mut Game,
    mark: &mut u64,
    workshop: Option<&Workshop>,
    action: impl FnOnce(&mut Game),
) -> bool {
    if workshop.is_some() {
        return false;
    }
    *mark = game.log().total();
    action(game);
    true
}

impl Focusable for GameView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<GameView>) -> impl IntoElement {
        let hurt = self.news().any(|k| matches!(k, LogKind::Hurt | LogKind::Death));
        self.sprites.begin(window);
        let map = map(&self.game, &mut self.sprites, hurt);
        let panel = panel(&self.game, &mut self.sprites, self.workshop.as_ref());
        self.sprites.end(window);
        let title = format!("blah · seed {}", self.game.world().seed());

        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &MoveNorth, _, cx| this.act(|g| _ = g.step(Dir::North), cx)))
            .on_action(cx.listener(|this, _: &MoveSouth, _, cx| this.act(|g| _ = g.step(Dir::South), cx)))
            .on_action(cx.listener(|this, _: &MoveEast, _, cx| this.act(|g| _ = g.step(Dir::East), cx)))
            .on_action(cx.listener(|this, _: &MoveWest, _, cx| this.act(|g| _ = g.step(Dir::West), cx)))
            .on_action(cx.listener(|this, _: &Wait, _, cx| this.act(|g| _ = g.wait(), cx)))
            .on_action(cx.listener(|this, _: &ToggleSneak, _, cx| this.act(|g| _ = g.toggle_sneak(), cx)))
            .on_action(cx.listener(|this, _: &RecoverFocus, _, cx| this.act(|g| _ = g.recover_focus(), cx)))
            .on_action(cx.listener(|this, _: &Unleash, _, cx| this.act(|g| _ = g.unleash(), cx)))
            .on_action(cx.listener(|this, _: &OpenWorkshop, _, cx| {
                if this.workshop.is_none() {
                    this.workshop = Some(Workshop::new(this.game.maneuver().clone()));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &CancelWorkshop, _, cx| {
                if this.workshop.take().is_some() {
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &PrepareWorkshop, _, cx| this.prepare_workshop(cx)))
            .on_action(cx.listener(|this, _: &WorkshopWeapon, _, cx| this.edit_workshop(Edit::Weapon, cx)))
            .on_action(cx.listener(|this, _: &WorkshopDelivery, _, cx| this.edit_workshop(Edit::Delivery, cx)))
            .on_action(cx.listener(|this, _: &WorkshopAim, _, cx| this.edit_workshop(Edit::Aim, cx)))
            .on_action(cx.listener(|this, _: &WorkshopPayload, _, cx| this.edit_workshop(Edit::Payload, cx)))
            .on_action(cx.listener(|this, _: &WorkshopElement, _, cx| this.edit_workshop(Edit::Element, cx)))
            .on_action(cx.listener(|this, _: &WorkshopTrigger, _, cx| this.edit_workshop(Edit::Trigger, cx)))
            .on_action(cx.listener(|this, _: &WorkshopShape, _, cx| this.edit_workshop(Edit::Shape, cx)))
            .on_action(cx.listener(|this, _: &WorkshopPowerUp, _, cx| this.edit_workshop(Edit::PowerUp, cx)))
            .on_action(cx.listener(|this, _: &WorkshopPowerDown, _, cx| this.edit_workshop(Edit::PowerDown, cx)))
            .on_action(
                cx.listener(|this, _: &WorkshopDaggerExample, _, cx| this.edit_workshop(Edit::DaggerExample, cx)),
            )
            .on_action(cx.listener(|this, _: &WorkshopStaffExample, _, cx| this.edit_workshop(Edit::StaffExample, cx)))
            .size_full()
            .bg(rgb(SCREEN))
            .font_family(FONT)
            .text_size(px(TEXT_SIZE))
            .line_height(px(LINE))
            .text_color(rgb(TEXT))
            .px(px(GAP))
            .pb(px(GAP))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(TITLE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(SMALL_SIZE))
                    .text_color(rgb(FAINT))
                    .child(title),
            )
            .child(div().flex().flex_row().gap(px(GAP)).child(map).child(panel))
            .child(div().h(px(GAP)))
            .child(log(&self.game, self.mark))
    }
}

/// The viewport: terrain, the monsters on it, and the wanderer at the center, tinted red if just hurt.
fn map(game: &Game, sprites: &mut SpriteCache, hurt: bool) -> Div {
    let (px0, py0) = game.position();
    let world = game.world();
    let rows = (-VIEW_RADIUS..=VIEW_RADIUS).map(|dy| {
        let cells: Vec<Div> = (-VIEW_RADIUS..=VIEW_RADIUS)
            .map(|dx| {
                let (x, y) = (px0 + dx, py0 + dy);
                let tile = world.tile(x, y);
                if dx == 0 && dy == 0 {
                    wanderer_cell(&tile, hurt)
                } else if let Some(monster) = game.monster_at(x, y) {
                    monster_cell(&tile, (x, y), monster, sprites)
                } else {
                    terrain_cell(&tile, (x, y))
                }
            })
            .collect();
        div().flex().flex_row().children(cells)
    });
    frame().size(px(MAP)).flex().flex_col().children(rows.collect::<Vec<_>>())
}

/// A bare tile: its biome's color, shaded a little per tile, with the biome's glyph.
fn terrain_cell(tile: &Tile, at: (i64, i64)) -> Div {
    let biome = tile.biome.biome();
    tile_base(biome.color, at)
        .text_color(color(glyph_shade(biome.color)))
        .child(SharedString::from(biome.glyph.to_string()))
}

/// The wanderer's tile: an `@` on darkened ground, or on red the turn it was hurt.
fn wanderer_cell(tile: &Tile, hurt: bool) -> Div {
    let bg = if hurt { rgb(HURT_TILE) } else { color(scale(tile.biome.biome().color, 0.3)) };
    div()
        .size(px(TILE))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .text_color(rgb(WANDERER))
        .text_size(px(GLYPH))
        .line_height(px(TILE))
        .font_weight(FontWeight::BOLD)
        .child("@")
}

/// A monster's own sprite on its tile, with a health pip under it once it's hurt.
fn monster_cell(tile: &Tile, at: (i64, i64), monster: &Monster, sprites: &mut SpriteCache) -> Div {
    let image = sprites.get(Key::Monster(monster.id), &monster.sprite);
    let cell = tile_base(tile.biome.biome().color, at).relative().child(img(image).size(px(SPRITE_PX)));
    let max = monster.max_hp().max(1);
    if monster.hp >= max {
        return cell;
    }
    const PIP_W: f32 = 26.0;
    let fill = PIP_W * monster.hp.max(0) as f32 / max as f32;
    cell.child(
        div()
            .absolute()
            .bottom(px(1.0))
            .left(px((TILE - PIP_W) / 2.0 - 1.0))
            .w(px(PIP_W + 2.0))
            .h(px(4.0))
            .p(px(1.0))
            .bg(rgb(PIP_BACK))
            .child(div().h_full().w(px(fill.max(1.0))).bg(rgb(PIP))),
    )
}

/// An empty tile in `base`, shaded by a hash of its coordinates so a field of one biome isn't flat.
fn tile_base(base: [u8; 3], (x, y): (i64, i64)) -> Div {
    div()
        .size(px(TILE))
        .flex()
        .items_center()
        .justify_center()
        .bg(color(scale(base, jitter(x, y))))
        .text_size(px(GLYPH))
        .line_height(px(TILE))
}

/// A brightness factor within about 5% of 1, fixed per tile.
fn jitter(x: i64, y: i64) -> f32 {
    // Neighbors hash apart, so the shading looks like grain rather than stripes. Shading is only looks, so it
    // takes a fixed key rather than the world's seed.
    let h = hash_point(0, x, y);
    1.0 + ((h % 11) as f32 - 5.0) * 0.01
}

/// The status panel: the wanderer, where they are, and what lives nearby.
fn panel(game: &Game, sprites: &mut SpriteCache, workshop: Option<&Workshop>) -> Div {
    let panel = frame().w(px(PANEL)).h(px(MAP)).p(px(INSET)).flex().flex_col();
    if let Some(workshop) = workshop {
        return panel.child(workshop_panel(workshop, game));
    }
    panel
        .child(wanderer(game))
        .child(rule())
        .child(place(game))
        .child(rule())
        .child(combat_status(game))
        .child(rule())
        .child(nearby(game, sprites))
        .child(div().flex_grow())
        .child(
            div()
                .flex()
                .flex_col()
                .text_color(rgb(FAINT))
                .text_size(px(SMALL_SIZE))
                .line_height(px(SMALL_LINE))
                .child("Arrows / hjkl / wasd move · bump attacks")
                .child("M edit · C sneak · R recover · F unleash")
                .child("Space / . wait · Q quit"),
        )
}

fn combat_status(game: &Game) -> Div {
    let maneuver = game.maneuver();
    let tracking = game.monsters_in_view().filter(|monster| monster.alerted).count();
    div()
        .flex()
        .flex_col()
        .child(bar("FP", game.wanderer().focus, MAX_FOCUS, HEADING))
        .child(pair("Maneuver", format!("{} · {}F", maneuver.name, maneuver.technique.focus_cost())))
        .child(pair("Sneaking", if game.sneaking() { "on".into() } else { "off".into() }))
        .child(pair(
            "Observers",
            if tracking == 0 { "none tracking you".into() } else { format!("{tracking} tracking you") },
        ))
        .child(combat_preview(game))
}

fn combat_preview(game: &Game) -> Div {
    if game.maneuver().technique.delivery == Delivery::Ground {
        return match game.preview_area() {
            Ok(preview) => div()
                .flex()
                .flex_col()
                .child(small_line(format!(
                    "Area r{}: {} targets, {} total damage",
                    preview.radius, preview.targets, preview.damage
                )))
                .child(colored_small_line(
                    format!("{} Focus; {} self damage (can kill)", preview.focus, preview.self_damage),
                    HP,
                )),
            Err(reason) => small_line(format!("Area preview: {reason}")),
        };
    }

    for direction in Dir::ALL {
        if let Ok(preview) = game.preview_attack(direction) {
            let direction = match direction {
                Dir::North => "N",
                Dir::South => "S",
                Dir::East => "E",
                Dir::West => "W",
            };
            let mut notes = Vec::new();
            if preview.opening {
                notes.push("opening");
            }
            if preview.penetration_possible {
                notes.push("penetration");
            }
            let notes = if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(", ")) };
            return div()
                .flex()
                .flex_col()
                .child(small_line(format!("{direction}: {}–{} on hit; misses do 0", preview.min_hit, preview.max_hit)))
                .child(small_line(format!("{} Focus{notes}", preview.focus)));
        }
    }
    small_line("Adjacent preview: no target")
}

fn workshop_panel(workshop: &Workshop, game: &Game) -> Div {
    let maneuver = workshop.draft();
    let technique = maneuver.technique;
    let reason = workshop.reason().or_else(|| (!game.can_prepare()).then_some("Nearby creatures prevent preparation."));
    let validation_message = match reason {
        Some(reason) => format!("Cannot prepare: {reason}"),
        None => "Core validates; safe to prepare when alone.".into(),
    };
    let validation_color = if reason.is_some() { HP } else { 0x8c_e0_6a };
    let mut panel = div()
        .flex()
        .flex_col()
        .child(heading("WORKSHOP · PAUSED"))
        .child(pair("Name", maneuver.name.clone()))
        .child(pair("1 Weapon", maneuver.weapon.name().into()))
        .child(pair("2 Delivery", delivery_name(technique.delivery).into()))
        .child(pair("3 Aim", aim_name(technique.aim).into()))
        .child(pair("4 Payload", if technique.burst.is_some() { "on" } else { "off" }.into()));
    if let Some(burst) = technique.burst {
        panel = panel
            .child(pair("5 Element", element_name(burst.element).into()))
            .child(pair("6 Release", trigger_name(burst.trigger).into()))
            .child(pair("7 Shape", shape_name(burst.shape)))
            .child(pair("+/- Power", burst.power.to_string()));
    }
    panel = panel
        .child(pair("Focus cost", format!("{} per use", technique.focus_cost())))
        .child(colored_small_line(validation_message, validation_color));
    if let Ok(damage) = maneuver.backlash() {
        panel = panel
            .child(colored_small_line(format!("Area also hits you: up to {damage} Health."), HP))
            .child(small_line("No safe center. This can kill you."));
    }
    panel
        .child(small_line("Enter prepares only with no creatures in view; Esc discards."))
        .child(div().h(px(6.0)))
        .child(small_line("1 weapon · 2 delivery · 3 aim · 4 payload"))
        .child(small_line("5 element · 6 trigger · 7 shape"))
        .child(small_line(format!("+ / - power (1–{MAX_BURST_POWER}) · area radius 1–{MAX_AREA_RADIUS}")))
        .child(small_line("0 dagger example · 9 staff inferno"))
}

fn delivery_name(delivery: Delivery) -> &'static str {
    match delivery {
        Delivery::Weapon => "Weapon",
        Delivery::Touch => "Touch",
        Delivery::Ground => "Ground",
    }
}

fn aim_name(aim: Aim) -> &'static str {
    match aim {
        Aim::Direct => "Direct",
        Aim::Opening => "Opening",
    }
}

fn element_name(element: Element) -> &'static str {
    match element {
        Element::Force => "Force",
        Element::Fire => "Fire",
    }
}

fn trigger_name(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Contact => "Contact",
        Trigger::Penetration => "Penetration",
    }
}

fn shape_name(shape: Shape) -> String {
    match shape {
        Shape::Point => "Point".into(),
        Shape::Area(radius) => format!("Area {radius}"),
    }
}

fn small_line(text: impl Into<SharedString>) -> Div {
    div().text_size(px(SMALL_SIZE)).line_height(px(SMALL_LINE)).text_color(rgb(DIM)).child(text.into())
}

fn colored_small_line(text: impl Into<SharedString>, tint: u32) -> Div {
    div().text_size(px(SMALL_SIZE)).line_height(px(SMALL_LINE)).text_color(rgb(tint)).child(text.into())
}

fn wanderer(game: &Game) -> Div {
    let w = game.wanderer();
    div()
        .flex()
        .flex_col()
        .child(heading("WANDERER"))
        .child(bar("HP", w.hp, w.max_hp(), HP))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(24.0))
                .child(pair("Attack", w.stats.attack.to_string()).flex_1())
                .child(pair("Defense", w.stats.defense.to_string()).flex_1()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(24.0))
                .child(pair("Turn", game.turn().to_string()).flex_1())
                .child(pair("Deaths", game.deaths().to_string()).flex_1()),
        )
}

/// The region's name, then the biome underfoot, which can differ at a region's ragged edges.
fn place(game: &Game) -> Div {
    let (x, y) = game.position();
    let biome = game.biome();
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(LINE))
                .truncate()
                .text_color(rgb(HEADING))
                .font_weight(FontWeight::BOLD)
                .child(capitalized(&game.region().name)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .h(px(LINE))
                .child(div().size(px(12.0)).bg(color(biome.color)).border_1().border_color(rgb(FRAME)))
                .child(div().text_color(rgb(DIM)).child(biome.name))
                .child(div().flex_grow())
                .child(div().text_color(rgb(FAINT)).child(format!("{x}, {y}"))),
        )
}

/// The species in view, nearest first, each with how it differs from its kin in a region visited before.
fn nearby(game: &Game, sprites: &mut SpriteCache) -> Div {
    let species: Vec<SpeciesId> = game.nearby_species().into_iter().take(NEARBY).collect();
    let list = div().flex().flex_col().gap(px(6.0)).child(heading("NEARBY"));
    if species.is_empty() {
        return list.child(div().text_color(rgb(FAINT)).child("nothing in sight"));
    }
    list.children(species.into_iter().filter_map(|id| nearby_entry(game, id, sprites)))
}

fn nearby_entry(game: &Game, id: SpeciesId, sprites: &mut SpriteCache) -> Option<Div> {
    let species = game.species(id)?;
    let count = game.monsters_in_view().filter(|m| m.species == id).count();
    let image = sprites.get(Key::Species(id), &species.sprite);
    let card = div()
        .flex_none()
        .size(px(CARD))
        .flex()
        .items_center()
        .justify_center()
        .bg(color(species.biome.biome().color))
        .child(img(image).size(px(SPRITE_PX)));
    let name = div()
        .h(px(18.0))
        .line_height(px(18.0))
        .flex()
        .flex_row()
        .gap(px(8.0))
        .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::BOLD).child(species.name.clone()))
        .child(div().flex_none().text_color(rgb(DIM)).child(if count > 1 {
            format!("×{count}")
        } else {
            String::new()
        }))
        // Monsters spawn from the region of their own tile, so a neighbor's population can wander into view.
        .when(id.region != game.region().id, |row| {
            row.child(div().flex_none().text_color(rgb(FAINT)).child("visiting"))
        });
    let description = div()
        .text_size(px(SMALL_SIZE))
        .line_height(px(SMALL_LINE))
        .text_color(rgb(DIM))
        .line_clamp(1)
        .child(species.description.clone());
    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(name)
        .child(description)
        .child(deltas(game.compare(id)));
    Some(div().flex().flex_row().gap(px(10.0)).child(card).child(text))
}

/// The strongest few trait changes against the clade elsewhere, as arrows: "heat ▲▲  water ▼".
fn deltas(comparison: Option<Comparison>) -> Div {
    let row = div().flex().flex_row().flex_wrap().gap_x(px(12.0)).text_size(px(13.0)).line_height(px(16.0));
    let Some(comparison) = comparison else {
        return row.child(div().text_color(rgb(HEADING)).child("new to you"));
    };
    let mut marked: Vec<_> = comparison.deltas.into_iter().filter(|d| d.marks != 0).collect();
    if marked.is_empty() {
        return row.child(div().text_color(rgb(FAINT)).child("as before"));
    }
    marked.sort_by(|a, b| b.delta.abs().total_cmp(&a.delta.abs()));
    row.children(marked.into_iter().take(DELTAS).map(|d| {
        let (arrow, tint) = if d.marks > 0 { ('▲', MORE) } else { ('▼', LESS) };
        let arrows: String = std::iter::repeat_n(arrow, usize::from(d.marks.unsigned_abs())).collect();
        div()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .child(div().text_color(rgb(TEXT)).child(trait_label(d.gene)))
            .child(div().text_color(rgb(tint)).child(arrows))
    }))
}

/// A gene's name, short enough for three to share a line.
const fn trait_label(gene: Gene) -> &'static str {
    match gene {
        Gene::Size => "size",
        Gene::Limbs => "limbs",
        Gene::Covering => "covering",
        Gene::Heat => "heat",
        Gene::Cold => "cold",
        Gene::Water => "water",
        Gene::Aggression => "fierce",
        Gene::Hue => "hue",
    }
}

/// The last lines of the log, the latest action's bright and the rest dim, colored by what they report.
fn log(game: &Game, mark: u64) -> Div {
    let log = game.log();
    let entries: Vec<_> = log.last(LOG_LINES).collect();
    // The log total of the first line shown, counting from 0.
    let first = log.total() - entries.len() as u64;
    let rows = entries.into_iter().enumerate().map(|(i, entry)| {
        let new = first + i as u64 >= mark;
        div()
            .h(px(LINE))
            .flex()
            .flex_row()
            .gap(px(8.0))
            .whitespace_nowrap()
            .overflow_hidden()
            .child(div().text_color(rgb(if new { HEADING } else { FAINT })).child("›"))
            .child(
                div()
                    .text_color(rgb(tint(entry.kind, new)))
                    .when(new && entry.kind == LogKind::Death, |d| d.font_weight(FontWeight::BOLD))
                    .child(SharedString::from(entry.text.clone())),
            )
    });
    frame().w_full().h(px(LOG)).p(px(INSET)).flex().flex_col().justify_end().children(rows.collect::<Vec<_>>())
}

/// A bordered window, the retro kind.
fn frame() -> Div {
    div().flex_none().bg(rgb(SCREEN)).border(px(BORDER)).border_color(rgb(FRAME))
}

fn heading(text: &'static str) -> Div {
    div().h(px(LINE)).text_color(rgb(HEADING)).font_weight(FontWeight::BOLD).child(text)
}

fn rule() -> Div {
    div().my(px(6.0)).h(px(1.0)).w_full().bg(rgb(FAINT))
}

fn pair(label: &'static str, value: String) -> Div {
    div()
        .h(px(LINE))
        .flex()
        .flex_row()
        .justify_between()
        .child(div().text_color(rgb(DIM)).child(label))
        .child(div().min_w_0().truncate().text_color(rgb(TEXT)).child(value))
}

/// A segmented gauge: label, ten cells filled in proportion, and the value over its maximum.
fn bar(label: &'static str, value: i32, max: i32, fill: u32) -> Div {
    const CELLS: i32 = 10;
    let max = max.max(1);
    let filled = (value.clamp(0, max) * CELLS + max - 1) / max;
    let cells = (0..CELLS).map(|i| div().w(px(14.0)).h(px(12.0)).bg(rgb(if i < filled { fill } else { EMPTY_BAR })));
    let low = value * 3 <= max;
    div()
        .h(px(LINE))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(div().w(px(24.0)).text_color(rgb(DIM)).child(label))
        .child(div().flex().flex_row().gap(px(2.0)).children(cells.collect::<Vec<_>>()))
        .child(div().flex_grow())
        .child(div().text_color(rgb(if low { HP } else { TEXT })).child(format!("{value}/{max}")))
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
    let s = |c: u8| (f32::from(c) * k).round().min(255.0) as u8;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workshop_blocks_all_play_actions_until_closed() {
        let mut game = Game::inferno_trial(42).unwrap();
        let workshop = Workshop::new(game.maneuver().clone());
        let before = game.clone();
        let mut mark = 999;
        let actions: [fn(&mut Game); 5] = [
            |g| {
                g.step(Dir::East);
            },
            |g| {
                g.wait();
            },
            |g| {
                g.toggle_sneak();
            },
            |g| {
                g.recover_focus();
            },
            |g| {
                g.unleash();
            },
        ];
        for action in actions {
            assert!(!apply_play_action(&mut game, &mut mark, Some(&workshop), action));
            assert_eq!(game, before);
            assert_eq!(mark, 999);
        }
        assert!(apply_play_action(&mut game, &mut mark, None, |g| {
            g.unleash();
        }));
        assert_eq!(game.turn(), before.turn() + 1);
        assert_eq!(mark, before.log().total());
    }

    #[test]
    fn tile_shading_is_slight_and_fixed() {
        for (x, y) in [(0, 0), (1, 0), (-5, 9), (i64::MIN, i64::MAX)] {
            let k = jitter(x, y);
            assert!((0.94..=1.06).contains(&k), "{k}");
            assert_eq!(k, jitter(x, y));
        }
    }
}
