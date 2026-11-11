//! The game window: a title strip, the map viewport on the left, the status panel on the right, and the
//! message log along the bottom.
//!
//! Everything is drawn from the [`Game`] and optional workshop or commitment panel on each render. The view
//! remembers draft/selection state until prepared, committed, or cancelled, the log mark, and cached sprites.

use gpui::{
    App, Context, Div, FocusHandle, Focusable, FontWeight, Pixels, Rgba, SharedString, Window, div, img, prelude::*,
    px, rgb,
};

use blah_core::{
    ADAPTATION_CAPACITY, Adaptation, Aim, Comparison, Delivery, Dir, Element, Game, Gene, Herb, HerbInfo, LandmarkInfo,
    LandmarkKind, LearnedMove, LogKind, MAX_AREA_RADIUS, MAX_BURST_POWER, Monster, MovePreview, Shape, SpeciesId, Tile,
    Trigger, VIEW_RADIUS, Weapon, hash_point,
};

use crate::commitment::{self, CommitmentPanel, Enter as CommitmentEnter};
use crate::sprites::{Key, SPRITE_PX, SpriteCache};
use crate::workshop::{Edit, Workshop};
use crate::{
    Camp, CancelWorkshop, ChartEncounter, ClaimCache, CycleEarnedMove, CycleRelics, CycleTrade, DrinkDraught,
    EarnedMoveEast, EarnedMoveNorth, EarnedMoveSouth, EarnedMoveWest, Gather, MoveEast, MoveNorth, MoveSouth, MoveWest,
    OpenCommitment, OpenWorkshop, PrepareWorkshop, RecoverFocus, TechniqueEast, TechniqueNorth, TechniqueSouth,
    TechniqueWest, ToggleSneak, TradeWeapon, Unleash, UseLandmark, UseSalve, Wait, WorkshopAim, WorkshopDaggerExample,
    WorkshopDelivery, WorkshopElement, WorkshopPayload, WorkshopPowerDown, WorkshopPowerUp, WorkshopShape,
    WorkshopStaffExample, WorkshopTrigger, WorkshopWeapon,
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

/// The three earned-move controls; the weapon slot resolves equipment at use time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EarnedMoveSlot {
    #[default]
    Weapon,
    Stealth,
    Focus,
}

impl EarnedMoveSlot {
    const fn next(self) -> Self {
        match self {
            Self::Weapon => Self::Stealth,
            Self::Stealth => Self::Focus,
            Self::Focus => Self::Weapon,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Weapon => "Weapon",
            Self::Stealth => "Stealth",
            Self::Focus => "Focus",
        }
    }
}

/// Resolve a slot only when it is used, so the weapon slot always follows current equipment.
fn earned_move_for_slot(slot: EarnedMoveSlot, weapon: Weapon) -> LearnedMove {
    match slot {
        EarnedMoveSlot::Weapon => LearnedMove::for_weapon(weapon),
        EarnedMoveSlot::Stealth => LearnedMove::SlipAway,
        EarnedMoveSlot::Focus => LearnedMove::Pulse,
    }
}

fn cycle_earned_move_slot(slot: &mut EarnedMoveSlot, modal_open: bool) -> bool {
    if modal_open {
        return false;
    }
    *slot = slot.next();
    true
}

/// Window width: map, panel, and the gaps around them.
pub const WINDOW_WIDTH: Pixels = px(GAP + MAP + GAP + PANEL + GAP);
/// Window height: title strip, map, log, and the gaps around them.
pub const WINDOW_HEIGHT: Pixels = px(TITLE + MAP + GAP + LOG + GAP);

const FONT: &str = "Menlo";
const TEXT_SIZE: f32 = 15.0;
const SMALL_SIZE: f32 = 12.0;
const SMALL_LINE: f32 = 15.0;
/// Menlo's advance width as a fraction of its size (1233/2048 units).
const MENLO_ADVANCE: f32 = 0.602;
/// Small-text characters that fit across the panel inside its border and padding.
const PANEL_CHARS: usize = ((PANEL - 2.0 * (INSET + BORDER)) / (SMALL_SIZE * MENLO_ADVANCE)) as usize;
/// Gap between a NEARBY card and its text.
const CARD_GAP: f32 = 10.0;
/// Small-text characters that fit beside a NEARBY card.
const NEARBY_CHARS: usize =
    ((PANEL - 2.0 * (INSET + BORDER) - CARD - CARD_GAP) / (SMALL_SIZE * MENLO_ADVANCE)) as usize;

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
        (LogKind::Region | LogKind::Learning, true) => HEADING,
        (LogKind::Region | LogKind::Learning, false) => 0x86_70_3a,
    }
}

/// The one window's root view.
pub struct GameView {
    game: Game,
    save: Option<crate::save::Save>,
    save_error: Option<String>,
    focus: FocusHandle,
    /// [`Log::total`](blah_core::Log::total) before the latest action: lines past it are that action's.
    mark: u64,
    sprites: SpriteCache,
    workshop: Option<Workshop>,
    commitment: Option<CommitmentPanel>,
    move_slot: EarnedMoveSlot,
    trade_slot: usize,
}

impl GameView {
    /// A view of `game`, taking keyboard focus so the movement keys reach it. Log lines past `mark` (a
    /// [`Log::total`](blah_core::Log::total)) show as new.
    pub fn new(
        game: Game,
        save: Option<crate::save::Save>,
        mark: u64,
        window: &mut Window,
        cx: &mut Context<GameView>,
    ) -> GameView {
        let focus = cx.focus_handle();
        window.focus(&focus);
        GameView {
            game,
            save,
            save_error: None,
            focus,
            mark,
            sprites: SpriteCache::default(),
            workshop: None,
            commitment: None,
            move_slot: EarnedMoveSlot::default(),
            trade_slot: 0,
        }
    }

    fn act(&mut self, action: impl FnOnce(&mut Game), cx: &mut Context<GameView>) {
        let mut result = Ok(());
        let modal_open = self.workshop.is_some() || self.commitment.is_some();
        if apply_play_action(&mut self.game, &mut self.mark, modal_open, |game| {
            result = crate::save::commit_action(game, &mut self.save, action);
        }) {
            self.save_error = result.err();
            cx.notify();
        }
    }

    fn use_earned_move(&mut self, direction: Dir, cx: &mut Context<GameView>) {
        let learned_move = earned_move_for_slot(self.move_slot, self.game.maneuver().weapon);
        self.act(
            |game| {
                let _ = game.use_move(learned_move, direction);
            },
            cx,
        );
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
        let mut prepared = false;
        let result = crate::save::commit_action(&mut self.game, &mut self.save, |game| {
            prepared = workshop.prepare(game).is_ok();
        });
        self.save_error = result.as_ref().err().cloned();
        if result.is_ok() && prepared {
            self.workshop = None;
        }
        cx.notify();
    }

    fn cycle_relics(&mut self, cx: &mut Context<GameView>) {
        if self.workshop.is_some() || self.commitment.is_some() {
            return;
        }
        let mut game_error = None;
        let result = crate::save::commit_action(&mut self.game, &mut self.save, |game| {
            game_error = game.cycle_relics().err().map(str::to_owned);
        });
        self.save_error = result.err().or(game_error);
        cx.notify();
    }

    fn open_commitment(&mut self, cx: &mut Context<GameView>) {
        if self.workshop.is_some()
            || self.commitment.is_some()
            || !self.game.can_prepare()
            || self.game.adaptation_offers().next().is_none()
        {
            return;
        }
        self.commitment = Some(CommitmentPanel::default());
        cx.notify();
    }

    fn navigate_commitment(&mut self, direction: isize, cx: &mut Context<GameView>) {
        let offers = self.game.adaptation_offers().count();
        if let Some(commitment) = &mut self.commitment {
            commitment.navigate(direction, offers);
            cx.notify();
        }
    }

    fn enter_panel(&mut self, cx: &mut Context<GameView>) {
        let Some(commitment) = &mut self.commitment else {
            self.prepare_workshop(cx);
            return;
        };
        let enter = commitment.enter(self.game.adaptation_offers().count());
        match enter {
            CommitmentEnter::Empty | CommitmentEnter::Confirmed => cx.notify(),
            CommitmentEnter::Commit(index) => self.commit_adaptation(index, cx),
        }
    }

    fn commit_adaptation(&mut self, index: usize, cx: &mut Context<GameView>) {
        let Some(adaptation) = self.game.adaptation_offers().nth(index) else {
            self.commitment = None;
            cx.notify();
            return;
        };

        self.mark = self.game.log().total();
        let mut committed = false;
        let mut game_error = None;
        let save_result = crate::save::commit_action(&mut self.game, &mut self.save, |game| {
            match game.commit_adaptation(adaptation) {
                Ok(()) => committed = true,
                Err(error) => game_error = Some(error),
            }
        });
        self.save_error = save_result.as_ref().err().cloned().or_else(|| game_error.map(str::to_owned));
        if save_result.is_ok() && committed {
            self.commitment = None;
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

/// Shared by every gameplay key. Open panels must not advance the world behind them.
fn apply_play_action(game: &mut Game, mark: &mut u64, modal_open: bool, action: impl FnOnce(&mut Game)) -> bool {
    if modal_open {
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
        let panel = panel(
            &self.game,
            &mut self.sprites,
            self.workshop.as_ref(),
            self.commitment.as_ref(),
            self.move_slot,
            self.trade_slot,
        );
        self.sprites.end(window);
        let title = self.save_error.clone().unwrap_or_else(|| {
            let slot = self
                .save
                .as_ref()
                .and_then(|s| s.path.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unsaved demo".into());
            format!("blah · seed {} · {slot}", self.game.world().seed())
        });

        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &MoveNorth, _, cx| {
                if this.commitment.is_some() {
                    this.navigate_commitment(-1, cx);
                } else {
                    this.act(|g| _ = g.step(Dir::North), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveSouth, _, cx| {
                if this.commitment.is_some() {
                    this.navigate_commitment(1, cx);
                } else {
                    this.act(|g| _ = g.step(Dir::South), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &MoveEast, _, cx| this.act(|g| _ = g.step(Dir::East), cx)))
            .on_action(cx.listener(|this, _: &MoveWest, _, cx| this.act(|g| _ = g.step(Dir::West), cx)))
            .on_action(cx.listener(|this, _: &TechniqueNorth, _, cx| this.act(|g| _ = g.use_technique(Dir::North), cx)))
            .on_action(cx.listener(|this, _: &TechniqueSouth, _, cx| this.act(|g| _ = g.use_technique(Dir::South), cx)))
            .on_action(cx.listener(|this, _: &TechniqueEast, _, cx| this.act(|g| _ = g.use_technique(Dir::East), cx)))
            .on_action(cx.listener(|this, _: &TechniqueWest, _, cx| this.act(|g| _ = g.use_technique(Dir::West), cx)))
            .on_action(cx.listener(|this, _: &EarnedMoveNorth, _, cx| this.use_earned_move(Dir::North, cx)))
            .on_action(cx.listener(|this, _: &EarnedMoveSouth, _, cx| this.use_earned_move(Dir::South, cx)))
            .on_action(cx.listener(|this, _: &EarnedMoveEast, _, cx| this.use_earned_move(Dir::East, cx)))
            .on_action(cx.listener(|this, _: &EarnedMoveWest, _, cx| this.use_earned_move(Dir::West, cx)))
            .on_action(cx.listener(|this, _: &CycleEarnedMove, _, cx| {
                if cycle_earned_move_slot(&mut this.move_slot, this.workshop.is_some() || this.commitment.is_some()) {
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Wait, _, cx| this.act(|g| _ = g.wait(), cx)))
            .on_action(cx.listener(|this, _: &ClaimCache, _, cx| this.act(|g| _ = g.claim_cache(), cx)))
            .on_action(cx.listener(|this, _: &Camp, _, cx| this.act(|g| _ = g.camp(), cx)))
            .on_action(cx.listener(|this, _: &UseSalve, _, cx| this.act(|g| _ = g.use_salve(), cx)))
            .on_action(cx.listener(|this, _: &Gather, _, cx| this.act(|g| _ = g.gather(), cx)))
            .on_action(cx.listener(|this, _: &DrinkDraught, _, cx| this.act(|g| _ = g.drink_draught(), cx)))
            .on_action(cx.listener(|this, _: &UseLandmark, _, cx| this.act(|g| _ = g.use_site(), cx)))
            .on_action(cx.listener(|this, _: &ChartEncounter, _, cx| this.act(|g| _ = g.chart_encounter(), cx)))
            .on_action(cx.listener(|this, _: &CycleRelics, _, cx| this.cycle_relics(cx)))
            .on_action(cx.listener(|this, _: &CycleTrade, _, cx| {
                if this.workshop.is_none() && this.commitment.is_none() {
                    this.trade_slot = this.trade_slot.wrapping_add(1);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &TradeWeapon, _, cx| {
                let item = trade_candidate(&this.game, this.trade_slot).unwrap_or(blah_core::WeaponItem::Dagger);
                this.act(|g| _ = g.trade_weapon(item), cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSneak, _, cx| this.act(|g| _ = g.toggle_sneak(), cx)))
            .on_action(cx.listener(|this, _: &RecoverFocus, _, cx| this.act(|g| _ = g.recover_focus(), cx)))
            .on_action(cx.listener(|this, _: &Unleash, _, cx| this.act(|g| _ = g.unleash(), cx)))
            .on_action(cx.listener(|this, _: &OpenWorkshop, _, cx| {
                if this.workshop.is_none() && this.commitment.is_none() {
                    this.workshop = Some(Workshop::from_game(&this.game));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &OpenCommitment, _, cx| this.open_commitment(cx)))
            .on_action(cx.listener(|this, _: &CancelWorkshop, _, cx| {
                let cancelled_commitment = commitment::cancel(&mut this.commitment);
                if this.workshop.take().is_some() || cancelled_commitment {
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &PrepareWorkshop, _, cx| this.enter_panel(cx)))
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
                    .text_color(rgb(if self.save_error.is_some() { 0xff_70_70 } else { FAINT }))
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
                let cell = if dx == 0 && dy == 0 {
                    wanderer_cell(&tile, hurt)
                } else if let Some(monster) = game.monster_at(x, y) {
                    monster_cell(&tile, (x, y), monster, sprites, game.cache_at(x, y).is_some())
                } else if let Some(object) = game.encounter_tile((x, y)) {
                    encounter_cell(&tile, (x, y), object)
                } else if let Some(cache) = game.cache_at(x, y) {
                    cache_cell(&tile, (x, y), cache)
                } else if let Some(landmark) = game.landmark_at(x, y) {
                    landmark_cell(&tile, (x, y), landmark)
                } else if (x, y) == game.home() {
                    home_cell(&tile, (x, y))
                } else if let Some(herb) = game.herb_at(x, y) {
                    herb_cell(&tile, (x, y), herb)
                } else {
                    terrain_cell(&tile, (x, y))
                };
                if game.encounter_danger_at((x, y)) > 0 {
                    cell.bg(rgb(0x70_18_10)).border_2().border_color(rgb(0xff_60_30))
                } else {
                    cell
                }
            })
            .collect();
        div().flex().flex_row().children(cells)
    });
    frame().size(px(MAP)).flex().flex_col().children(rows.collect::<Vec<_>>())
}

/// An unclaimed cache uses an amber capital; a spent cache stays marked but dims.
fn cache_cell(tile: &Tile, at: (i64, i64), cache: blah_core::CacheInfo) -> Div {
    let claimed = cache.claimed;
    let glyph = match (cache.ruin, claimed) {
        (false, false) => "C",
        (false, true) => "c",
        (true, false) => "R",
        (true, true) => "r",
    };
    tile_base(tile.biome.biome().color, at)
        .border_2()
        .border_color(rgb(if claimed { DIM } else { HEADING }))
        .text_color(rgb(if claimed { DIM } else { HEADING }))
        .font_weight(FontWeight::BOLD)
        .child(glyph)
}

fn encounter_cell(tile: &Tile, at: (i64, i64), object: blah_core::EncounterTile) -> Div {
    use blah_core::EncounterTile;
    let (glyph, ready) = match object {
        EncounterTile::Warning => ("!", true),
        EncounterTile::Brazier(lit) => (if lit { "B" } else { "b" }, lit),
        EncounterTile::Salvage(ready) => (if ready { "A" } else { "a" }, ready),
        EncounterTile::Heart(ready) => (if ready { "E" } else { "e" }, ready),
        EncounterTile::Spring(ready) => (if ready { "W" } else { "w" }, ready),
    };
    tile_base(tile.biome.biome().color, at)
        .bg(rgb(0x20_12_0c))
        .border_2()
        .border_color(rgb(if ready { HEADING } else { DIM }))
        .text_color(rgb(if ready { HEADING } else { DIM }))
        .font_weight(FontWeight::BOLD)
        .child(glyph)
}

/// Landmarks use fixed glyphs; spent or regrowing sites are dim.
fn landmark_cell(tile: &Tile, at: (i64, i64), landmark: LandmarkInfo) -> Div {
    let tint = if landmark.ready { 0xa8_e0_aa } else { DIM };
    tile_base(tile.biome.biome().color, at)
        .bg(rgb(0x10_1a_12))
        .border_2()
        .border_color(rgb(tint))
        .text_color(rgb(tint))
        .font_weight(FontWeight::BOLD)
        .child(landmark.kind.glyph(landmark.ready).to_string())
}

/// Ready herbs use a bright sprig; a picked patch keeps a dim marker until it regrows.
fn herb_cell(tile: &Tile, at: (i64, i64), herb: HerbInfo) -> Div {
    tile_base(tile.biome.biome().color, at)
        .bg(rgb(0x10_1a_12))
        .border_1()
        .border_color(rgb(if herb.ready { 0xb6_f0_90 } else { DIM }))
        .text_color(rgb(if herb.ready { 0xb6_f0_90 } else { DIM }))
        .font_weight(FontWeight::BOLD)
        .child(if herb.ready { "*" } else { "·" })
}

/// A bare tile: its biome's color, shaded a little per tile, with the biome's glyph.
fn terrain_cell(tile: &Tile, at: (i64, i64)) -> Div {
    let biome = tile.biome.biome();
    tile_base(biome.color, at)
        .text_color(color(glyph_shade(biome.color)))
        .child(SharedString::from(biome.glyph.to_string()))
}

/// Home, where Focus recovers and death returns you: a bright marker on its own ground.
fn home_cell(tile: &Tile, at: (i64, i64)) -> Div {
    tile_base(tile.biome.biome().color, at)
        .border_2()
        .border_color(rgb(HEADING))
        .text_color(rgb(HEADING))
        .font_weight(FontWeight::BOLD)
        .child("H")
}

/// Which way home is and how many steps away, e.g. "12 N, 3 W", or "here".
fn home_hint((x, y): (i64, i64), (hx, hy): (i64, i64)) -> String {
    if (x, y) == (hx, hy) { "here (R rests)".into() } else { destination_hint((x, y), (hx, hy)) }
}

fn destination_hint((x, y): (i64, i64), (hx, hy): (i64, i64)) -> String {
    let (dx, dy) = (hx - x, hy - y);
    let mut parts = Vec::new();
    if dy != 0 {
        parts.push(format!("{} {}", dy.unsigned_abs(), if dy < 0 { "N" } else { "S" }));
    }
    if dx != 0 {
        parts.push(format!("{} {}", dx.unsigned_abs(), if dx < 0 { "W" } else { "E" }));
    }
    if parts.is_empty() { "here".into() } else { parts.join(", ") }
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
fn monster_cell(tile: &Tile, at: (i64, i64), monster: &Monster, sprites: &mut SpriteCache, cache: bool) -> Div {
    let image = sprites.get(Key::Monster(monster.id), &monster.sprite);
    let base = tile_base(tile.biome.biome().color, at);
    let base = if cache { base.border_2().border_color(rgb(HEADING)) } else { base };
    let cell = base.relative().child(img(image).size(px(SPRITE_PX)));
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
fn panel(
    game: &Game,
    sprites: &mut SpriteCache,
    workshop: Option<&Workshop>,
    commitment: Option<&CommitmentPanel>,
    move_slot: EarnedMoveSlot,
    trade_slot: usize,
) -> impl IntoElement + use<> {
    let panel = frame().id("status-panel").w(px(PANEL)).h(px(MAP)).p(px(INSET)).flex().flex_col().overflow_y_scroll();
    if let Some(workshop) = workshop {
        return panel.child(workshop_panel(workshop, game));
    }
    if let Some(commitment) = commitment {
        return panel.child(commitment_panel(commitment, game));
    }
    let mut panel = panel.child(wanderer(game)).child(rule());
    if game.encounter().is_some() {
        panel = panel.child(encounter_panel(game)).child(rule());
    }
    panel
        .child(trails_panel(game))
        .child(rule())
        .child(adaptations(game))
        .child(rule())
        .child(equipment(game, trade_slot))
        .child(rule())
        .child(relics_panel(game))
        .child(rule())
        .child(gathering(game))
        .child(rule())
        .child(cache_status(game))
        .child(rule())
        .child(landmark_status(game))
        .child(rule())
        .child(place(game))
        .child(rule())
        .child(combat_status(game, move_slot))
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
                .child("Arrows / hjkl / wasd · bump = free strike")
                .child("Shift + direction = prepared technique")
                .child("Tab slots · Alt + direction = earned move")
                .child("M edit · C sneak · R home rest · P camp")
                .child("V salve · G claim cache · F unleash")
                .child("E gather · T draught · P also brews")
                .child("X use landmark · Y cycle relics at home")
                .child("I choose trade · U trade at home")
                .child("Space / . wait · Q quit"),
        )
}

fn trade_candidate(game: &Game, slot: usize) -> Option<blah_core::WeaponItem> {
    let items: Vec<_> = game
        .inventory()
        .iter()
        .copied()
        .filter(|item| blah_core::WeaponItem::CACHE_REWARDS.contains(item) && *item != game.equipped_item())
        .collect();
    if items.is_empty() { None } else { Some(items[slot % items.len()]) }
}

fn equipment(game: &Game, trade_slot: usize) -> Div {
    let owned = game.inventory().iter().map(|item| item.name()).collect::<Vec<_>>().join(", ");
    div()
        .flex()
        .flex_col()
        .child(heading("EQUIPMENT"))
        .child(pair("Equipped", game.equipped_item().name().into()))
        .children(wrap(&format!("Owned: {owned}"), PANEL_CHARS).into_iter().map(small_line))
        .child(pair("Salves", format!("{}/{} · V to use", game.salves(), blah_core::SALVE_CAPACITY)))
        .children(
            wrap(
                &format!(
                    "Trade: {} · I choose, U exchange at home",
                    trade_candidate(game, trade_slot).map_or("no spare cache weapon", |item| item.name())
                ),
                PANEL_CHARS,
            )
            .into_iter()
            .map(small_line),
        )
}

fn gathering(game: &Game) -> Div {
    let mut panel = div().flex().flex_col().child(heading("HERB POUCH"));
    for herb in Herb::ALL {
        panel = panel.child(pair(herb.name(), format!("{}/{}", game.herbs(herb), blah_core::HERB_CAPACITY)));
    }
    panel = panel
        .child(pair("Herbalism", game.wanderer().herbalism.rank().name().into()))
        .child(small_line(format!(
            "Extra herb {}% · regrows {} turns{}",
            game.wanderer().herbalism.second_herb_chance(),
            game.wanderer().herbalism.herb_regrow_turns(),
            if game.wanderer().herbalism.quiet_gathering() { " · quiet" } else { "" }
        )))
        .child(pair("Draughts", format!("{}/{} · T to drink", game.draughts(), blah_core::DRAUGHT_CAPACITY)))
        .child(small_line(format!("P brews: 2 different herbs → {} Focus", game.wanderer().herbalism.draught_focus())));
    if let Some(info) = game.herb_at(game.position().0, game.position().1) {
        panel = panel.child(small_line(format!(
            "{} here · {}",
            info.herb.name(),
            if info.ready { "E to gather" } else { "regrowing" }
        )));
    }
    panel
}

fn trails_panel(game: &Game) -> Div {
    let mut panel = div().flex().flex_col().child(heading("FIELD SIGNS"));
    if game.encounter().is_none() {
        panel = panel.child(small_line("Z: trace the cinder smoke (authored arc)"));
    }
    if let Some(at) = game.cache_trail()
        && game.cache_at(at.0, at.1).is_none_or(|cache| !cache.claimed)
    {
        panel = panel.child(small_line(format!("Cache trail: {}", destination_hint(game.position(), at))));
    }
    for herb in Herb::ALL {
        if game.herbs(herb) == 0
            && let Some(at) = game.herb_trail(herb)
        {
            panel = panel.child(small_line(format!("{}: {}", herb.name(), destination_hint(game.position(), at))));
        }
    }
    panel.child(small_line("Directions only; scout the route and guards."))
}

fn encounter_lines(game: &Game) -> Vec<String> {
    use blah_core::{EncounterPhase, SentryCycle};
    let Some(info) = game.encounter() else { return Vec::new() };
    let def = info.definition;
    let mut lines = vec![format!("Warning/spring: {}", destination_hint(game.position(), def.entrance()))];
    if info.phase == EncounterPhase::Revealed {
        lines.extend([
            "Scorched lanes; a powerful warden waits.".into(),
            "X at ! investigates before commitment.".into(),
            "Approach or leave freely. No locked exit.".into(),
        ]);
    } else {
        for sentry in info.sentries {
            let state = if sentry.hp == 0 { "dead" } else { sentry.cycle.name() };
            lines.push(format!(
                "{}: {} HP, {state}",
                if sentry.kind == blah_core::SentryKind::Warden { "Warden" } else { "Tender" },
                sentry.hp
            ));
            if let SentryCycle::Aiming(dir) = sentry.cycle {
                lines.push(format!("{dir:?} red lane fires AFTER next action."));
            }
        }
        if info.phase == EncounterPhase::Resolved {
            lines.push(
                if info.reward_claimed { "Heart claimed; spring restored." } else { "X at E: claim Hearth ember." }
                    .into(),
            );
            lines.push(
                if info.spring_ready {
                    "X at W: refill salves and draughts."
                } else {
                    "Spring recovers after 300 turns."
                }
                .into(),
            );
        } else {
            lines.push(format!(
                "Lit braziers: {}. X shuts (4 Focus; 2 unseen).",
                info.braziers.iter().filter(|&&lit| lit).count()
            ));
            lines.push("Fire: up to 12, range 4. Sidestep red lanes.".into());
            lines.push("Two exposed actions; Shove interrupts.".into());
            lines.push(format!(
                "Alcove: {}{}",
                destination_hint(game.position(), def.salvage()),
                if info.salvage_claimed { " (taken)" } else { " (X: Hushstone)" }
            ));
            lines.push("Leave 7 tiles from the heart to abandon.".into());
        }
    }
    lines.into_iter().flat_map(|line| wrap(&line, PANEL_CHARS)).collect()
}

fn encounter_panel(game: &Game) -> Div {
    div().flex().flex_col().child(heading("CINDER WATCH")).children(encounter_lines(game).into_iter().map(small_line))
}

fn relics_panel(game: &Game) -> Div {
    let mut panel = div().flex().flex_col().child(heading("RELICS"));
    let attuned = game.attuned_relics();
    for (index, relic) in attuned.into_iter().enumerate() {
        let description =
            relic.map_or_else(|| "empty".to_owned(), |relic| format!("{} · {}", relic.name(), relic.effect()));
        panel = panel
            .children(wrap(&format!("Slot {}: {description}", index + 1), PANEL_CHARS).into_iter().map(small_line));
    }
    let owned = game.relics().iter().map(|relic| relic.name()).collect::<Vec<_>>();
    if owned.is_empty() {
        panel = panel.child(small_line("No relics owned."));
    } else {
        panel = panel.children(wrap(&format!("Owned: {}", owned.join(", ")), PANEL_CHARS).into_iter().map(small_line));
    }
    panel.child(small_line("Y cycles pairs at home; X uses a site."))
}

fn cache_status(game: &Game) -> Div {
    let (x, y) = game.position();
    let nearest = (-VIEW_RADIUS..=VIEW_RADIUS)
        .flat_map(|dy| (-VIEW_RADIUS..=VIEW_RADIUS).map(move |dx| (dx, dy)))
        .filter_map(|(dx, dy)| game.cache_at(x + dx, y + dy).map(|cache| (dx.abs().max(dy.abs()), dx, dy, cache)))
        .min_by_key(|(distance, dx, dy, _)| (*distance, *dy, *dx));
    let Some((distance, dx, dy, cache)) = nearest else {
        return div();
    };
    let at_cache = distance == 0;
    let status = if cache.claimed {
        "claimed".to_owned()
    } else if at_cache {
        "G to claim".to_owned()
    } else {
        format!("{distance} tiles · move there, then G")
    };
    let treasure = if cache.ruin {
        match cache.relic_reward {
            Some(relic) => format!("{} + {}", cache.treasure.name(), relic.name()),
            None => format!("{} + no new relic", cache.treasure.name()),
        }
    } else {
        cache.treasure.name().to_owned()
    };
    div()
        .flex()
        .flex_col()
        .child(heading(if cache.ruin { "GUARDED RUIN" } else { "CACHE" }))
        .child(pair("Status", status))
        .child(pair("Guards", format!("{}/{} alive", cache.guards_alive, cache.guards)))
        .children(wrap(&format!("Treasure: {treasure}"), PANEL_CHARS).into_iter().map(small_line))
        .child(small_line(format!("Direction: {}", if at_cache { "here".to_owned() } else { compass(dx, dy) })))
}

fn landmark_status(game: &Game) -> Div {
    let (x, y) = game.position();
    let nearest = game
        .known_landmarks()
        .map(|landmark| {
            let (dx, dy) = (landmark.position.0 - x, landmark.position.1 - y);
            (dx.abs().max(dy.abs()), dx, dy, landmark)
        })
        .min_by_key(|(distance, dx, dy, _)| (*distance, *dy, *dx));
    let Some((distance, dx, dy, landmark)) = nearest else {
        return div();
    };
    let status = if distance == 0 {
        if landmark.kind == LandmarkKind::Shrine && landmark.relic_reward.is_none() {
            "no new relic".to_owned()
        } else if landmark.ready {
            "X to use".to_owned()
        } else {
            "spent or regrowing".to_owned()
        }
    } else {
        format!("{distance} tiles · {}", compass(dx, dy))
    };
    let mut panel = div()
        .flex()
        .flex_col()
        .child(heading("LANDMARK"))
        .child(pair("Site", landmark.kind.name().into()))
        .child(pair("Status", status));
    if let Some(relic) = landmark.relic_reward
        && landmark.kind == LandmarkKind::Shrine
    {
        panel = panel.children(wrap(&format!("Reward: {}", relic.name()), PANEL_CHARS).into_iter().map(small_line));
    }
    panel
}

/// The eight-way compass point for an offset, e.g. "NE".
fn compass(dx: i64, dy: i64) -> String {
    let ns = match dy.signum() {
        -1 => "N",
        1 => "S",
        _ => "",
    };
    let ew = match dx.signum() {
        -1 => "W",
        1 => "E",
        _ => "",
    };
    format!("{ns}{ew}")
}

fn adaptations(game: &Game) -> Div {
    let held: Vec<_> = game.wanderer().adaptations.held().collect();
    let offers = game.adaptation_offers().next().is_some();
    let ready =
        if !game.can_prepare() { "Breakthrough ready (B) · return when clear" } else { "Breakthrough ready (B)" };
    let mut section = div()
        .flex()
        .flex_col()
        .child(heading("ADAPTATIONS"))
        .child(pair("Capacity", format!("{}/{} held", held.len(), ADAPTATION_CAPACITY)));
    if held.is_empty() {
        section = section.child(small_line("none committed"));
    } else {
        for adaptation in held {
            section = section.child(small_line(format!("· {}", adaptation.name())));
        }
    }
    if offers {
        section.children(wrap(ready, PANEL_CHARS).into_iter().map(|line| colored_small_line(line, HEADING)))
    } else {
        section
    }
}

fn commitment_panel(commitment: &CommitmentPanel, game: &Game) -> Div {
    let offers: Vec<Adaptation> = game.adaptation_offers().collect();
    let selected = commitment.selected(offers.len());
    let held: Vec<_> = game.wanderer().adaptations.held().collect();
    let mut panel = div()
        .flex()
        .flex_col()
        .child(heading("BREAKTHROUGH · PAUSED"))
        .child(pair("Capacity", format!("{}/{} held", held.len(), ADAPTATION_CAPACITY)));
    if held.is_empty() {
        panel = panel.child(small_line("Held: none"));
    } else {
        panel = panel.child(small_line("Held:"));
        for adaptation in held {
            panel = panel.child(small_line(format!("· {}", adaptation.name())));
        }
    }
    if offers.is_empty() {
        panel = panel.child(colored_small_line("No earned adaptations fit.", HP));
    } else {
        for (index, adaptation) in offers.iter().copied().enumerate() {
            let active = selected == Some(index);
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .mt(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(if active { HEADING } else { TEXT }))
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{} {}", if active { "›" } else { " " }, adaptation.name())),
                    )
                    .children(
                        [
                            format!("Benefit: {}", adaptation.benefit()),
                            format!("Cost: {}", adaptation.cost()),
                            format!("Rules out: {}", adaptation.rules_out()),
                        ]
                        .into_iter()
                        .flat_map(|detail| wrap(&detail, PANEL_CHARS))
                        .map(small_line)
                        .collect::<Vec<_>>(),
                    ),
            );
        }
    }
    let instructions = match (selected, commitment.is_confirmed()) {
        (Some(index), true) => format!("Enter again to commit {}.", offers[index].name()),
        (Some(_), false) => "Enter to confirm; Enter again commits.".into(),
        (None, _) => "No available option.".into(),
    };
    panel
        .child(div().h(px(8.0)))
        .children(
            wrap(&instructions, PANEL_CHARS)
                .into_iter()
                .map(|line| colored_small_line(line, if commitment.is_confirmed() { HEADING } else { DIM })),
        )
        .children(wrap("↑ / ↓ choose · Esc defers without advancing time", PANEL_CHARS).into_iter().map(small_line))
}

fn combat_status(game: &Game, move_slot: EarnedMoveSlot) -> Div {
    let maneuver = game.maneuver();
    let tracking = game.monsters_in_view().filter(|monster| monster.alerted).count();
    div()
        .flex()
        .flex_col()
        .child(bar("FP", game.wanderer().focus, game.wanderer().max_focus(), HEADING))
        .child(pair("Maneuver", format!("{} · {}F", maneuver.name, maneuver.technique.focus_cost())))
        .child(pair("Sneaking", if game.sneaking() { "on".into() } else { "off".into() }))
        .child(pair(
            "Observers",
            if tracking == 0 { "none tracking you".into() } else { format!("{tracking} tracking you") },
        ))
        .child(earned_move_status(game, move_slot))
        .child(combat_preview(game))
}

fn earned_move_status(game: &Game, slot: EarnedMoveSlot) -> Div {
    let learned_move = earned_move_for_slot(slot, game.maneuver().weapon);
    let lines = earned_move_lines(game, slot, learned_move)
        .into_iter()
        .fold(div().flex().flex_col(), |lines, line| lines.child(small_line(line)));
    div()
        .flex()
        .flex_col()
        .child(pair(
            "Move",
            format!("{} · {}F", learned_move.name(), learned_move.focus_cost_with_item(game.equipped_item())),
        ))
        .child(lines)
}

/// Move status split into lines that fit the panel, so no detail is clipped.
fn earned_move_lines(game: &Game, slot: EarnedMoveSlot, learned_move: LearnedMove) -> Vec<String> {
    let slot = slot.name();
    if !learned_move.unlocked(game.wanderer()) {
        return pack_line(format!("{slot} · Locked"), &[format!("needs {}", learned_move.requirement())]);
    }
    let mut first_error = None;
    for direction in Dir::ALL {
        match game.preview_move(learned_move, direction) {
            Ok(preview) => return ready_move_lines(slot, learned_move, direction, preview),
            Err(reason) => {
                first_error.get_or_insert(reason);
            }
        }
    }
    pack_line(format!("{slot} · Unavailable"), &[first_error.unwrap_or("no valid direction").to_owned()])
}

fn ready_move_lines(slot: &str, learned_move: LearnedMove, direction: Dir, preview: MovePreview) -> Vec<String> {
    let direction = match direction {
        Dir::North => "N",
        Dir::South => "S",
        Dir::East => "E",
        Dir::West => "W",
    };
    let mut details = Vec::new();
    let head = match learned_move {
        LearnedMove::SlipAway => {
            details.push("2 steps".to_owned());
            format!("{slot} · Ready · Alt+{direction}")
        }
        LearnedMove::Sweep => {
            details.push(format!("{} targets", preview.targets));
            details.push(format!("{}–{} if all hit", preview.min_hit, preview.max_hit));
            format!("{slot} · Ready · Alt+direction")
        }
        _ => {
            details.push(format!("{} target{}", preview.targets, if preview.targets == 1 { "" } else { "s" }));
            details.push(format!("{}–{} on hit", preview.min_hit, preview.max_hit));
            format!("{slot} · Ready · Alt+{direction}")
        }
    };
    if let Some((x, y)) = preview.destination {
        details.push(format!("you→{x},{y}"));
    }
    if let Some((x, y)) = preview.push {
        details.push(format!("push→{x},{y}"));
    } else if learned_move == LearnedMove::Shove {
        details.push("no room to push".to_owned());
    }
    if preview.focus_gain > 0 {
        details.push(format!("+{} Focus on hit", preview.focus_gain));
    }
    pack_line(head, &details)
}

/// Put the head on its own line, then pack details with " · " and word-wrap any that are too long.
fn pack_line(head: String, details: &[String]) -> Vec<String> {
    let mut lines = wrap(&head, PANEL_CHARS);
    let mut current = String::new();
    for detail in details {
        for part in wrap(detail, PANEL_CHARS) {
            let joined = if current.is_empty() { part.clone() } else { format!("{current} · {part}") };
            if joined.chars().count() <= PANEL_CHARS {
                current = joined;
            } else {
                lines.push(std::mem::replace(&mut current, part));
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Word-wrap to `width` characters, splitting only words that are longer than a whole line.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_owned();
        while word.chars().count() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let rest = word.split_off(word.char_indices().nth(width).map_or(word.len(), |(i, _)| i));
            lines.push(std::mem::replace(&mut word, rest));
        }
        if line.is_empty() {
            line = word;
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
fn combat_preview(game: &Game) -> Div {
    let mut previews = div().flex().flex_col();
    for direction in Dir::ALL {
        if let Ok(preview) = game.preview_strike(direction) {
            previews = previews.child(small_line(format!(
                "Bump {direction:?}: {}–{} on hit · 0 Focus",
                preview.min_hit, preview.max_hit
            )));
            break;
        }
    }
    if game.maneuver().technique.delivery == Delivery::Ground {
        return previews.child(match game.preview_area() {
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
        });
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
            return previews
                .child(small_line(format!("Shift+{direction}: {}–{} on hit; miss 0", preview.min_hit, preview.max_hit)))
                .child(small_line(format!("{} Focus{notes}", preview.focus)));
        }
    }
    previews.child(small_line("Adjacent technique: no target"))
}

fn workshop_panel(workshop: &Workshop, game: &Game) -> Div {
    let maneuver = workshop.draft();
    let technique = maneuver.technique;
    let reason = game
        .validate_maneuver(maneuver)
        .err()
        .or(workshop.reason())
        .or_else(|| (!game.can_prepare()).then_some("Nearby creatures prevent preparation."));
    let validation_message = match reason {
        Some(reason) => format!("Cannot prepare: {reason}"),
        None => "Core validates; safe to prepare here.".into(),
    };
    let validation_color = if reason.is_some() { HP } else { 0x8c_e0_6a };
    let mut panel = div()
        .flex()
        .flex_col()
        .child(heading("WORKSHOP · PAUSED"))
        .child(pair("Name", maneuver.name.clone()))
        .child(pair("1 Weapon", workshop.equipped().name().into()))
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
        .children(
            wrap(
                &format!("Owned: {}", game.inventory().iter().map(|item| item.name()).collect::<Vec<_>>().join(", ")),
                PANEL_CHARS,
            )
            .into_iter()
            .map(small_line),
        )
        .child(colored_small_line(validation_message, validation_color));
    if let Ok(damage) = game.maneuver_backlash(maneuver) {
        panel = panel
            .child(colored_small_line(format!("Area also hits you: up to {damage} Health."), HP))
            .child(small_line("No safe center. This can kill you."));
    }
    panel
        .child(small_line("Enter prepares at home or with no creatures in view; Esc discards."))
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
                .gap(px(12.0))
                .child(skill_rank(game.maneuver().weapon.name(), w.weapons.get(game.maneuver().weapon).rank()))
                .child(skill_rank("Focus", w.focus_skill.rank()))
                .child(skill_rank("Stealth", w.stealth.rank())),
        )
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

/// Each skill gets a bounded column, with its rank below the label rather than beside it.
fn skill_rank(label: &'static str, rank: blah_core::Rank) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(small_line(label))
        .child(div().h(px(LINE)).truncate().child(rank.name()))
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
        .child(pair("Home", home_hint(game.position(), game.home())))
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
    let description = wrap(&species.description, NEARBY_CHARS)
        .into_iter()
        .fold(div().flex().flex_col(), |lines, line| lines.child(small_line(line)));
    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(name)
        .child(description)
        .child(deltas(game.compare(id)));
    Some(div().flex().flex_row().gap(px(CARD_GAP)).child(card).child(text))
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
    let max = max.max(1);
    let cells = cell_fills(value, max).map(|filled| {
        div()
            .w(px(f32::from(CELL_W)))
            .h(px(12.0))
            .bg(rgb(EMPTY_BAR))
            .child(div().h_full().w(px(f32::from(filled))).bg(rgb(fill)))
    });
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

/// Cells in a gauge.
const CELLS: u8 = 10;
/// Width of one gauge cell, in pixels.
const CELL_W: u8 = 14;

/// Filled pixels per cell. The last filled cell is partial, so every point of change shows, and any
/// value above zero fills at least one pixel.
fn cell_fills(value: i32, max: i32) -> impl Iterator<Item = u8> {
    let max = i64::from(max.max(1));
    let width = i64::from(CELLS) * i64::from(CELL_W);
    let filled = (i64::from(value).clamp(0, max) * width + max - 1) / max;
    (0..CELLS).map(move |i| (filled - i64::from(i) * i64::from(CELL_W)).clamp(0, i64::from(CELL_W)) as u8)
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
    use blah_core::{LearnedMove, MAX_FOCUS};

    #[test]
    fn trade_selection_cycles_only_spare_cache_weapons() {
        use blah_core::WeaponItem;
        let game = Game::new(42);
        assert_eq!(trade_candidate(&game, 0), None);
        let mut state = serde_json::to_value(game.snapshot()).unwrap();
        state["inventory"]
            .as_array_mut()
            .unwrap()
            .extend([serde_json::json!("HeavyClub"), serde_json::json!("QuietStaff")]);
        let game = Game::from_snapshot(serde_json::from_value(state).unwrap()).unwrap();
        assert_eq!(trade_candidate(&game, 0), Some(WeaponItem::HeavyClub));
        assert_eq!(trade_candidate(&game, 1), Some(WeaponItem::QuietStaff));
        assert_eq!(trade_candidate(&game, 2), Some(WeaponItem::HeavyClub));
    }

    #[test]
    fn a_modal_blocks_all_play_actions_until_closed() {
        let mut game = Game::inferno_trial(42).unwrap();
        let before = game.clone();
        let mut mark = 999;
        let actions: [fn(&mut Game); 7] = [
            |g| {
                g.use_technique(Dir::East);
            },
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
            |g| {
                let _ = g.use_move(LearnedMove::Pulse, Dir::East);
            },
        ];
        for action in actions {
            assert!(!apply_play_action(&mut game, &mut mark, true, action));
            assert_eq!(game, before);
            assert_eq!(mark, 999);
        }
        let mut slot = EarnedMoveSlot::Weapon;
        assert!(!cycle_earned_move_slot(&mut slot, true));
        assert_eq!(slot, EarnedMoveSlot::Weapon);
        assert!(apply_play_action(&mut game, &mut mark, false, |g| {
            g.unleash();
        }));
        assert_eq!(game.turn(), before.turn() + 1);
        assert_eq!(mark, before.log().total());
    }

    #[test]
    fn commitment_panel_blocks_play_and_escape_defers_without_changing_time() {
        let mut game = Game::new(42);
        let before = game.clone();
        let mut mark = 999;
        assert!(!apply_play_action(&mut game, &mut mark, true, |g| {
            g.wait();
        }));
        assert_eq!(game, before);
        assert_eq!(mark, 999);

        let mut commitment = Some(CommitmentPanel::default());
        assert!(commitment::cancel(&mut commitment));
        assert_eq!(commitment, None);
        assert_eq!(game, before);
        assert_eq!(game.turn(), before.turn());
    }

    #[test]
    fn play_actions_can_choose_a_free_strike_then_the_prepared_technique() {
        let mut game = Game::composition_trial(42, blah_core::Maneuver::backstab_burst()).unwrap();
        let prepared = game.maneuver().clone();
        let mut mark = 0;
        let turn = game.turn();
        assert!(apply_play_action(&mut game, &mut mark, false, |g| {
            assert_eq!(g.step(Dir::East), blah_core::Step::Attacked);
        }));
        assert_eq!(game.turn(), turn + 1);
        assert_eq!(game.wanderer().focus, MAX_FOCUS);
        assert_eq!(game.maneuver(), &prepared);
        assert_eq!(game.last_attack().unwrap().burst, 0);
        assert!(!game.sneaking());
        let previous_log = game.log().total();
        assert!(apply_play_action(&mut game, &mut mark, false, |g| {
            assert_eq!(g.use_technique(Dir::East), blah_core::Step::Attacked);
        }));
        assert_eq!(mark, previous_log);
        assert_eq!(game.turn(), turn + 2);
        assert_eq!(game.wanderer().focus, MAX_FOCUS - prepared.technique.focus_cost());
        assert_eq!(game.maneuver(), &prepared);
    }

    #[test]
    fn earned_move_slots_cycle_without_advancing_the_game() {
        let game = Game::new(42);
        let before = game.clone();
        let mut slot = EarnedMoveSlot::default();
        for expected in [EarnedMoveSlot::Stealth, EarnedMoveSlot::Focus, EarnedMoveSlot::Weapon] {
            assert!(cycle_earned_move_slot(&mut slot, false));
            assert_eq!(slot, expected);
            assert_eq!(game, before);
        }
        assert!(!cycle_earned_move_slot(&mut slot, true));
        assert_eq!(slot, EarnedMoveSlot::Weapon);
        assert_eq!(game, before);
    }

    #[test]
    fn weapon_slot_resolves_from_current_equipment() {
        let dagger = Game::composition_trial(42, blah_core::Maneuver::default()).unwrap();
        let staff = Game::inferno_trial(42).unwrap();
        assert_eq!(
            earned_move_for_slot(EarnedMoveSlot::Weapon, dagger.maneuver().weapon),
            LearnedMove::for_weapon(dagger.maneuver().weapon)
        );
        assert_eq!(
            earned_move_for_slot(EarnedMoveSlot::Weapon, staff.maneuver().weapon),
            LearnedMove::for_weapon(staff.maneuver().weapon)
        );
        assert_ne!(
            earned_move_for_slot(EarnedMoveSlot::Weapon, dagger.maneuver().weapon),
            earned_move_for_slot(EarnedMoveSlot::Weapon, staff.maneuver().weapon)
        );
    }

    #[test]
    fn panel_line_width_matches_the_panel_geometry() {
        // 324 px of text area at 12 px Menlo.
        assert_eq!(PANEL_CHARS, 44);
    }

    fn fits(lines: &[String]) {
        for line in lines {
            assert!(line.chars().count() <= PANEL_CHARS, "{line:?} is wider than {PANEL_CHARS}");
        }
    }

    #[test]
    fn every_ready_move_fits_the_panel_without_dropping_details() {
        let far = -9_999_999_i64;
        for learned_move in [
            LearnedMove::Withdraw,
            LearnedMove::Reach,
            LearnedMove::Shove,
            LearnedMove::Sweep,
            LearnedMove::SlipAway,
            LearnedMove::Pulse,
        ] {
            for push in [None, Some((far, far))] {
                let preview = MovePreview {
                    focus: learned_move.focus_cost(),
                    focus_gain: 99,
                    targets: 4,
                    min_hit: 999,
                    max_hit: 9_999,
                    destination: Some((far, far)),
                    push,
                };
                for slot in [EarnedMoveSlot::Weapon, EarnedMoveSlot::Stealth, EarnedMoveSlot::Focus] {
                    let lines = ready_move_lines(slot.name(), learned_move, Dir::West, preview);
                    fits(&lines);
                    let all = lines.join(" · ");
                    assert!(all.contains("Ready"), "{all}");
                    assert!(all.contains(&format!("you→{far},{far}")), "{all}");
                    assert!(all.contains("+99 Focus on hit"), "{all}");
                    if push.is_some() {
                        assert!(all.contains(&format!("push→{far},{far}")), "{all}");
                    }
                }
            }
        }
    }

    #[test]
    fn long_reasons_wrap_instead_of_being_cut() {
        let reason = "Not enough Focus for the learned move. Bump strikes remain free.";
        let lines = pack_line("Weapon · Unavailable".into(), &[reason.into(), "x".repeat(PANEL_CHARS * 2 + 3)]);
        fits(&lines);
        let words: Vec<_> = lines[1..].join(" ").split_whitespace().map(str::to_owned).collect();
        assert_eq!(words[..reason.split_whitespace().count()].join(" "), reason);
        assert_eq!(lines.concat().matches('x').count(), PANEL_CHARS * 2 + 3);
    }

    #[test]
    fn real_move_status_lines_fit_from_locked_to_ready() {
        let fresh = Game::new(42);
        for slot in [EarnedMoveSlot::Weapon, EarnedMoveSlot::Stealth, EarnedMoveSlot::Focus] {
            let lines = earned_move_lines(&fresh, slot, earned_move_for_slot(slot, fresh.maneuver().weapon));
            fits(&lines);
            assert!(lines[0].ends_with("Locked"), "{lines:?}");
        }
        let mut caster = Game::composition_trial(42, blah_core::Maneuver::backstab_burst()).unwrap();
        for _ in 0..3 {
            caster.use_technique(Dir::East);
        }
        assert!(LearnedMove::Pulse.unlocked(caster.wanderer()));
        let lines = earned_move_lines(&caster, EarnedMoveSlot::Focus, LearnedMove::Pulse);
        fits(&lines);
        assert!(lines[0].starts_with("Focus · Ready"), "{lines:?}");
        assert!(lines.join(" ").contains("Focus on hit"), "{lines:?}");
    }
    #[test]
    fn gauges_show_every_point_and_stay_in_bounds() {
        let fills = |value, max| cell_fills(value, max).collect::<Vec<_>>();
        assert_eq!(fills(0, 26), vec![0; 10]);
        assert_eq!(fills(26, 26), vec![CELL_W; 10]);
        assert_eq!(fills(99, 26), vec![CELL_W; 10]);
        assert_eq!(fills(-5, 26), vec![0; 10]);
        assert!(fills(1, 10_000).iter().map(|&p| u32::from(p)).sum::<u32>() >= 1);
        for max in [1, 7, 24, 26, 30, 101] {
            let mut previous = 0;
            for value in 0..=max {
                let total: u32 = fills(value, max).iter().map(|&p| u32::from(p)).sum();
                assert!(value == 0 || total > previous, "{value}/{max} looks the same as {}", value - 1);
                previous = total;
            }
        }
    }

    #[test]
    fn longest_creature_description_wraps_within_three_lines() {
        assert_eq!(NEARBY_CHARS, 38);
        let worst = "mid-sized eight-legged feathered crawler, heat-hardy, cold-hardy, amphibious, territorial";
        let lines = wrap(worst, NEARBY_CHARS);
        assert!(lines.len() <= 3, "{lines:?}");
        assert!(lines.iter().all(|line| line.chars().count() <= NEARBY_CHARS), "{lines:?}");
        assert_eq!(lines.join(" "), worst);
        for seed in 0..8 {
            let game = Game::new(seed);
            for id in game.nearby_species() {
                let species = game.species(id).unwrap();
                assert!(wrap(&species.description, NEARBY_CHARS).len() <= 3, "{}", species.description);
            }
        }
    }
    #[test]
    fn home_hint_names_direction_and_steps() {
        assert_eq!(home_hint((0, 0), (0, 0)), "here (R rests)");
        assert_eq!(home_hint((5, 5), (2, -7)), "12 N, 3 W");
        assert_eq!(home_hint((0, 0), (4, 1)), "1 S, 4 E");
        assert_eq!(home_hint((0, 0), (0, 9)), "9 S");
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
