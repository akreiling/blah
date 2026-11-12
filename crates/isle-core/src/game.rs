//! The game: a character, the creatures, what lies on the ground, and the round that resolves a command.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use blah_core::{Phenotype, Temperament};
use serde::{Deserialize, Serialize};

use crate::command::{self, Command, Dir, Instant, MAX_STEPS, Parsed, Target};
use crate::data::{Armor, Bow, Item, Kind, SHOP, Spell, Stats, Weapon, sale_price, treasure};
use crate::deep::Deep;
use crate::herb::{HerbPatch, SECTOR as HERB_SECTOR};
use crate::landmark::{Landmark, SECTOR};
use crate::map::{LevelId, Place, Pos, Terrain, Tile, Zone, distance};
use crate::practice::{Practice, Practices, rank_steps};
use crate::relic::Relic;
use crate::rng::Rng;
use crate::town::{Town, Towns};
use crate::wildlife::{LifeCache, Wildlife};

/// Cells a character or creature sees.
pub const SIGHT: i32 = 6;
/// Cells a creature notices you from.
const NOTICE: i32 = 7;
/// Rounds a creature keeps hunting after losing sight of you.
const PATIENCE: u32 = 12;
/// How far from home woods beasts follow you; the town lies beyond it.
const WILD_LEASH: i32 = 7;
/// How far from home Deep life follows you.
const DEEP_LEASH: i32 = 14;
/// A loud footstep can be followed along at most five traversable Deep cells.
const TREMOR_LOUD_RANGE: usize = 5;
/// A footstep taken while still hidden carries only two traversable cells.
const TREMOR_QUIET_RANGE: usize = 2;
/// Dim cave light reaches two cells from its living source.
const CAVE_LIGHT_RADIUS: i32 = 2;
/// Rounds before an emptied spawn point raises another creature.
const RESPAWN: u64 = 60;
/// Rounds before a Deep's den has a new keeper.
const DEN_RESPAWN: u64 = 400;
/// Things the sack holds.
pub const SACK: usize = 12;
/// Log lines kept.
const LOG_KEEP: usize = 300;
/// Outstanding or spent injury ledgers retained for individual damage sources.
const MAX_INJURY_SOURCES: usize = 64;
/// Total XP needed to reach adventuring ranks 1 through 10. Each step after rank 2 roughly doubles.
const EXPERIENCE_RANKS: [u32; 10] = [0, 100, 300, 700, 1_500, 3_100, 6_300, 12_700, 25_500, 51_100];
const MAX_FOCUS: i32 = 24;

/// How a log line should stand out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tone {
    /// Plain narration.
    Info,
    /// Your blow or shot landed.
    Hit,
    /// You missed, or something missed you.
    Miss,
    /// You were hurt.
    Hurt,
    /// Something died.
    Kill,
    /// You died.
    Death,
    /// Gains: gold, adventuring ranks, healing.
    Gain,
    /// The game talking to you.
    System,
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// How it should look.
    pub tone: Tone,
    /// What it says.
    pub text: String,
}

/// Where creation stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Waiting for a name.
    Name,
    /// Playing.
    Play,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InjurySource {
    creature_id: u32,
    threat: u32,
    capacity: i32,
    spent: i32,
    remaining: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HealedInjury {
    creature_id: u32,
    threat: u32,
    amount: u32,
}

/// The character.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    /// What they are called.
    pub name: String,
    /// Adventuring rank, from 1 to 10. Rank is a summary of earned XP, not a power source.
    pub level: u32,
    /// Experience earned from killing creatures.
    pub xp: u32,
    /// Strength: melee hits and damage.
    pub str: i32,
    /// Dexterity: armor, arrows, hiding.
    pub dex: i32,
    /// Intellect: spells.
    pub int: i32,
    /// Constitution: health.
    pub con: i32,
    /// Health now.
    pub hp: i32,
    /// Health at full.
    pub max_hp: i32,
    /// Focus available for spells.
    pub focus: i32,
    /// Focus capacity, raised by Focus practice.
    pub max_focus: i32,
    /// Doing-based practice, retained across equipment changes and death.
    pub practice: Practices,
    /// Which map they are on.
    pub at: LevelId,
    /// Where on it.
    pub pos: Pos,
    /// The hand weapon.
    pub weapon: Weapon,
    /// The bow, if carried.
    pub bow: Option<Bow>,
    /// Arrows in the quiver.
    pub arrows: u32,
    /// Armor worn.
    pub armor: Option<Armor>,
    /// Whether the ring of warding is worn.
    pub ring: bool,
    /// The one relic worn, separate from armor and the ring.
    #[serde(default)]
    pub relic: Option<Relic>,
    /// Everything else carried.
    pub sack: Vec<Item>,
    /// Coins.
    pub gold: u32,
    /// Whether an arrow is on the string.
    pub nocked: bool,
    /// The spell chanted and waiting to be cast.
    pub chanted: Option<Spell>,
    /// Whether hidden in the shadows.
    pub hidden: bool,
    /// Times fallen.
    pub deaths: u32,
    /// Rounds the Focus ward has left.
    #[serde(default)]
    pub shield: u32,
    /// Focus paid for the ward's next actual prevention.
    #[serde(default)]
    shield_focus_credit: u32,
    /// Bounded injury provenance, consumed by every source of healing.
    #[serde(default)]
    injuries: Vec<InjurySource>,
}

/// The bonus a stat gives.
pub const fn modifier(stat: i32) -> i32 {
    (stat - 10).div_euclid(2)
}

impl Player {
    fn roll(name: String, rng: &mut Rng) -> Player {
        let mut stat = || {
            let mut dice = [rng.range(1, 6), rng.range(1, 6), rng.range(1, 6), rng.range(1, 6)];
            dice.sort_unstable();
            dice[1] + dice[2] + dice[3]
        };
        let (str, dex, int, con) = (stat(), stat(), stat(), stat());
        let max_hp = (24 + modifier(con)).max(6);
        Player {
            name,
            level: 1,
            xp: 0,
            str,
            dex,
            int,
            con,
            hp: max_hp,
            max_hp,
            focus: MAX_FOCUS,
            max_focus: MAX_FOCUS,
            practice: Practices::default(),
            at: LevelId::Surface,
            pos: Town::START,
            weapon: Weapon::ShortSword,
            bow: Some(Bow::Shortbow),
            arrows: 12,
            armor: Some(Armor::Leather),
            ring: false,
            relic: None,
            sack: vec![Item::Weapon(Weapon::Staff), Item::Potion],
            gold: 50,
            nocked: false,
            chanted: None,
            hidden: false,
            deaths: 0,
            shield: 0,
            shield_focus_credit: 0,
            injuries: Vec::new(),
        }
    }

    /// What an attack roll must reach to hit you.
    pub fn ac(&self) -> i32 {
        10 + modifier(self.dex) + self.armor.map_or(0, Armor::protection) + if self.ring { 2 } else { 0 }
    }

    fn melee_bonus(&self) -> i32 {
        modifier(self.str)
            + self.weapon.accuracy()
            + rank_steps(self.practice.get(Practice::weapon(self.weapon)).rank())
    }

    fn missile_bonus(&self) -> i32 {
        modifier(self.dex)
            + rank_steps(self.practice.get(Practice::Archery).rank())
            + if self.relic == Some(Relic::HuntersToken) { 2 } else { 0 }
    }

    fn refresh_focus_capacity(&mut self) {
        self.max_focus = self.practice.get(Practice::Focus).focus_capacity()
            - if self.relic == Some(Relic::Stillstone) { 4 } else { 0 };
        self.focus = self.focus.min(self.max_focus);
    }

    /// Protection from the active Shield, including practice and the currently worn relic.
    pub fn ward_protection(&self, missile: bool) -> i32 {
        if self.shield == 0 {
            return 0;
        }
        (if missile { 6 } else { 3 })
            + rank_steps(self.practice.get(Practice::Warding).rank())
            + if self.relic == Some(Relic::Stillstone) { 2 } else { 0 }
    }

    /// Percentage chance of a hiding attempt before adjacent watchers and cave light refuse it.
    pub fn hiding_chance(&self, watchers: i32) -> i32 {
        let base = (60 + modifier(self.dex) * 5 + 5 * rank_steps(self.practice.get(Practice::Stealth).rank())
            - watchers * 15)
            .clamp(0, 100);
        (base - if self.relic == Some(Relic::HuntersToken) { 15 } else { 0 }).max(0)
    }

    fn baseline_max_hp(&self) -> i32 {
        (24 + modifier(self.con)).max(6)
    }

    fn record_injury(&mut self, creature_id: u32, threat: u32, capacity: i32, amount: i32) {
        let Some(amount) = (amount.max(0) > 0).then_some(amount.max(0)) else { return };
        if let Some(source) = self.injuries.iter_mut().find(|source| source.creature_id == creature_id) {
            let accepted = amount.min((source.capacity - source.spent).max(0));
            source.spent += accepted;
            source.remaining += accepted;
        } else if self.injuries.len() < MAX_INJURY_SOURCES {
            let accepted = amount.min(capacity.max(0));
            if accepted > 0 {
                self.injuries.push(InjurySource {
                    creature_id,
                    threat,
                    capacity: capacity.max(0),
                    spent: accepted,
                    remaining: accepted,
                });
            }
        }
    }

    fn consume_injuries(&mut self, amount: i32) -> Vec<HealedInjury> {
        let mut left = amount.max(0);
        let mut healed = Vec::new();
        for source in &mut self.injuries {
            if left == 0 {
                break;
            }
            let restored = source.remaining.min(left);
            if restored > 0 {
                source.remaining -= restored;
                left -= restored;
                healed.push(HealedInjury {
                    creature_id: source.creature_id,
                    threat: source.threat,
                    amount: restored as u32,
                });
            }
        }
        healed
    }

    /// Total experience needed for the next adventuring rank, if there is one.
    pub fn next_level(&self) -> Option<u32> {
        EXPERIENCE_RANKS.get(self.level as usize).copied()
    }
}

/// A creature on the island.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    /// Stable identity.
    pub id: u32,
    /// What it is.
    pub kind: Kind,
    /// Regional identity and genome, for evolved wildlife.
    pub wild: Option<Wildlife>,
    /// Banked speed, in tenths of an action.
    energy: i32,
    /// Which map.
    pub at: LevelId,
    /// Where.
    pub pos: Pos,
    /// Health now.
    pub hp: i32,
    /// Where it rose, and wanders near.
    pub home: Pos,
    /// Whether it is hunting you.
    pub hunting: bool,
    /// Where it last saw you or heard one of your footsteps.
    pub last_seen: Option<Pos>,
    /// Rounds since it last saw or heard you.
    pub lost: u32,
    /// Its remembered position came from tremorsense, not sight or provocation.
    #[serde(default)]
    listening: bool,
    /// Rounds it stays stunned.
    #[serde(default)]
    pub stunned: u32,
    /// Remaining useful damage this individual can teach across physical and magical attacks.
    #[serde(default)]
    learning_damage_left: i32,
    /// Remaining incoming-damage evidence for Vitality.
    #[serde(default)]
    vitality_lessons_left: i32,
    /// Remaining concealed-movement, opening, and escape lessons.
    #[serde(default)]
    stealth_lessons_left: u8,
    /// Remaining effective healing, warding, and binding lessons.
    #[serde(default)]
    utility_lessons_left: u8,
    /// An attack revealed the player; merely running away from that reveal is not a stealth escape.
    #[serde(default)]
    revealed_by_attack: bool,
    spawn: Option<usize>,
}

impl Creature {
    /// This individual's combat numbers, rather than a fixed species template.
    pub fn stats(&self) -> Stats {
        self.wild.as_ref().map_or_else(|| self.kind.stats(), Wildlife::stats)
    }

    /// The individual phenotype, if this creature comes from evolved life.
    pub fn phenotype(&self) -> Option<Phenotype> {
        self.wild.as_ref().map(Wildlife::phenotype)
    }

    /// Cave adaptations inherited from this individual's Deep lineage and phenotype.
    pub fn cave_traits(&self) -> crate::wildlife::CaveTraits {
        self.wild.as_ref().map_or_else(crate::wildlife::CaveTraits::default, Wildlife::cave_traits)
    }

    fn speed(&self) -> i32 {
        self.phenotype().map_or(blah_core::ACTION_COST, |p| p.stats.speed)
    }

    fn teaches_anything(&self) -> bool {
        self.speed() > 0 && !self.phenotype().is_some_and(|p| p.temperament == Temperament::Skittish)
    }

    fn spend_damage(&mut self, damage: i32) -> u32 {
        let spent = damage.max(0).min(self.hp.max(0)).min(self.learning_damage_left.max(0));
        self.learning_damage_left -= spent;
        spent as u32
    }

    fn spend_vitality(&mut self, damage: i32) -> u32 {
        let spent = damage.max(0).min(self.vitality_lessons_left.max(0));
        self.vitality_lessons_left -= spent;
        spent as u32
    }

    fn spend_stealth(&mut self, lessons: u8) -> u32 {
        let spent = lessons.min(self.stealth_lessons_left);
        self.stealth_lessons_left -= spent;
        u32::from(spent)
    }

    fn spend_utility_lesson(&mut self) -> bool {
        if self.utility_lessons_left == 0 {
            false
        } else {
            self.utility_lessons_left -= 1;
            true
        }
    }
}

/// Things lying on one cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pile {
    /// Which map.
    pub at: LevelId,
    /// Where.
    pub pos: Pos,
    /// What lies there.
    pub items: Vec<Item>,
}

/// What has been seen of one floor of a Deep, a bit per cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct DeepMemory {
    at: LevelId,
    seen: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SpawnState {
    at: LevelId,
    index: usize,
    den: bool,
    occupant: Option<u32>,
    ready: u64,
}

/// One accepted player step, retained through the round so tremorsense can follow its real source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Footstep {
    at: LevelId,
    pos: Pos,
    quiet: bool,
}

/// A creature in sight, with the letter commands may name it by.
#[derive(Clone, Copy, Debug)]
pub struct Sighted<'a> {
    /// The letter: A for the nearest.
    pub label: char,
    /// The creature.
    pub creature: &'a Creature,
}

/// The whole island.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Game {
    seed: u64,
    terrain: Terrain,
    towns: Towns,
    revival: Pos,
    rng: Rng,
    round: u64,
    phase: Phase,
    player: Player,
    creatures: Vec<Creature>,
    next_id: u32,
    piles: Vec<Pile>,
    spawns: Vec<SpawnState>,
    /// Cells ever seen on each floor of a Deep visited.
    deeps: Vec<DeepMemory>,
    /// On the surface, 16×16 chunks of remembered cells, including negative coordinates.
    explored: BTreeMap<i32, BTreeMap<i32, [u64; 4]>>,
    /// Only sites actually seen; opened caches never replenish.
    #[serde(default)]
    landmarks: Vec<Landmark>,
    /// Only discovered patches, with permanent harvest state.
    #[serde(default)]
    herbs: Vec<HerbPatch>,
    /// At most three accepted player steps from the current/most recently resolved round.
    #[serde(default)]
    footsteps: Vec<Footstep>,
    log: Vec<Line>,
    log_total: u64,
    wild_ready: u64,
    #[serde(skip)]
    life: LifeCache,
}

impl Game {
    /// A fresh island for `seed`, waiting for a character's name.
    pub fn new(seed: u64) -> Game {
        let mut rng = Rng::new(seed);
        let player = Player::roll(String::new(), &mut rng);
        let terrain = Terrain::new(seed);
        let mut towns = Towns::new(seed, terrain);
        towns.ensure(seed, terrain, Town::START);
        let mut game = Game {
            seed,
            terrain,
            revival: towns.home.respawn(),
            towns,
            rng,
            round: 0,
            phase: Phase::Name,
            player,
            creatures: Vec::new(),
            next_id: 1,
            piles: Vec::new(),
            spawns: Vec::new(),
            deeps: Vec::new(),
            explored: BTreeMap::new(),
            landmarks: Vec::new(),
            herbs: Vec::new(),
            footsteps: Vec::new(),
            log: Vec::new(),
            log_total: 0,
            wild_ready: 0,
            life: LifeCache::default(),
        };
        game.say(Tone::System, "You wash ashore on the Isle. What is your name?");
        game
    }

    // ----- Reading the state -----

    /// The seed the island was made from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Rounds played.
    pub fn round_number(&self) -> u64 {
        self.round
    }

    /// Where creation stands.
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// The character.
    pub fn player(&self) -> &Player {
        &self.player
    }

    /// The map the character is on.
    pub fn level(&self) -> Place {
        Place::new(self.player.at, self.terrain, self.towns.nearby(self.terrain, self.player.pos), self.player.pos)
    }

    /// Every creature alive.
    pub fn creatures(&self) -> &[Creature] {
        &self.creatures
    }

    /// The pile on `pos` of the character's map, if any.
    pub fn pile_at(&self, pos: Pos) -> Option<&Pile> {
        self.piles.iter().find(|p| p.at == self.player.at && p.pos == pos)
    }

    /// Wilderness sites the character has discovered, including opened caches.
    pub fn landmarks(&self) -> &[Landmark] {
        &self.landmarks
    }

    /// A discovered site on the character's current map; never reveals an unseen site.
    pub fn landmark_at(&self, pos: Pos) -> Option<&Landmark> {
        (self.player.at == LevelId::Surface).then(|| self.landmarks.iter().find(|site| site.pos == pos)).flatten()
    }

    /// Discovered herb patches, including those already picked.
    pub fn herbs(&self) -> &[HerbPatch] {
        &self.herbs
    }

    /// A discovered patch on the current map. Picked patches retain their harvest record.
    pub fn herb_at(&self, pos: Pos) -> Option<&HerbPatch> {
        (self.player.at == LevelId::Surface).then(|| self.herbs.iter().find(|patch| patch.pos == pos)).flatten()
    }

    /// Whether the character has ever seen `pos` on their map.
    pub fn remembers(&self, pos: Pos) -> bool {
        if self.player.at != LevelId::Surface {
            let Some(i) = self.level().deep().and_then(|d| d.cell_index(pos)) else { return false };
            return self.deeps.iter().any(|d| d.at == self.player.at && d.seen[i / 64] & 1 << (i % 64) != 0);
        }
        let (chunk, word, bit) = memory_cell(pos);
        self.explored.get(&chunk.0).and_then(|column| column.get(&chunk.1)).is_some_and(|words| words[word] & bit != 0)
    }

    /// Whether the character sees `pos` now.
    pub fn sees(&self, pos: Pos) -> bool {
        self.level().sees(self.player.pos, pos, SIGHT)
    }

    /// Whether living Deep life lights this walkable cell from within two cells and clear line of sight.
    pub fn illuminated(&self, pos: Pos) -> bool {
        if !matches!(self.player.at, LevelId::Deep(..)) {
            return false;
        }
        let level = self.level();
        level.tile(pos).walkable()
            && self.creatures.iter().any(|creature| {
                creature.at == self.player.at
                    && creature.hp > 0
                    && creature.cave_traits().luminous
                    && level.tile(creature.pos).walkable()
                    && distance(creature.pos, pos) <= CAVE_LIGHT_RADIUS
                    && level.sees(creature.pos, pos, CAVE_LIGHT_RADIUS)
            })
    }

    fn enforce_light(&mut self) {
        if self.player.hidden && self.illuminated(self.player.pos) {
            self.player.hidden = false;
            self.say(Tone::Info, "The glow drives you out of the shadows.");
        }
    }

    /// The log, oldest first; at most the last few hundred lines.
    pub fn log(&self) -> &[Line] {
        &self.log
    }

    /// Lines ever logged, including ones dropped from the front.
    pub fn log_total(&self) -> u64 {
        self.log_total
    }

    /// Creatures in sight, nearest first, lettered A, B, C.
    pub fn sighted(&self) -> Vec<Sighted<'_>> {
        let me = self.player.pos;
        let mut seen: Vec<&Creature> =
            self.creatures.iter().filter(|c| c.at == self.player.at && self.sees(c.pos)).collect();
        seen.sort_by_key(|c| (distance(me, c.pos), c.pos.1, c.pos.0, c.id));
        seen.into_iter()
            .take(26)
            .enumerate()
            .map(|(i, creature)| Sighted { label: (b'A' + i as u8) as char, creature })
            .collect()
    }

    /// The creature on `pos` of the character's map.
    pub fn creature_at(&self, pos: Pos) -> Option<&Creature> {
        self.creatures.iter().find(|c| c.at == self.player.at && c.pos == pos)
    }

    /// The service zone the character stands in.
    pub fn zone(&self) -> Zone {
        self.level().zone(self.player.pos)
    }

    // ----- Talking to the game -----

    fn say(&mut self, tone: Tone, text: impl Into<String>) {
        self.log.push(Line { tone, text: text.into() });
        self.log_total += 1;
        if self.log.len() > LOG_KEEP {
            self.log.drain(..self.log.len() - LOG_KEEP);
        }
    }

    /// Logs a line from outside the rules, such as the window explaining a key.
    pub fn note(&mut self, text: impl Into<String>) {
        self.say(Tone::System, text);
    }

    /// Answers a creation prompt. Does nothing once play has begun.
    pub fn answer(&mut self, line: &str) {
        let line = line.trim();
        match self.phase.clone() {
            Phase::Name => {
                let name: String = line.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect();
                let name = name.trim();
                if name.is_empty() || name.len() > 16 {
                    self.say(Tone::System, "A name of one to sixteen letters, please.");
                    return;
                }
                let mut chars = name.chars();
                let name: String = chars.next().into_iter().flat_map(char::to_uppercase).chain(chars).collect();
                self.player = Player::roll(name, &mut self.rng);
                self.phase = Phase::Play;
                let p = &self.player;
                let line = format!(
                    "{}: Str {} Dex {} Int {} Con {}, {} health and {} Focus.",
                    p.name, p.str, p.dex, p.int, p.con, p.max_hp, p.max_focus,
                );
                self.say(Tone::Gain, line);
                self.say(Tone::Info, format!("You stand in {}.", self.towns.home.name()));
                self.town_directions();
                self.say(Tone::System, "Type help for the commands. Each command you enter takes one round.");
                self.update_seen();
            }
            Phase::Play => {}
        }
    }

    /// Reads a typed line.
    pub fn parse(&self, line: &str) -> Result<Parsed, String> {
        command::parse(line)
    }

    /// Answers a command that takes no time.
    pub fn instant(&mut self, instant: Instant) {
        match instant {
            Instant::Help => {
                for line in command::HELP {
                    self.say(Tone::System, line);
                }
            }
            Instant::Inventory => {
                let p = &self.player;
                let mut lines = vec![format!(
                    "In hand: {}. Bow: {}. Worn: {}{}.",
                    p.weapon.name(),
                    p.bow.map_or("none".to_owned(), |b| format!("{} with {} arrows", b.name(), p.arrows)),
                    p.armor.map_or("no armor", Armor::name),
                    if p.ring { " and the ring of warding" } else { "" }
                )];
                lines.push(if p.sack.is_empty() {
                    format!("Your sack is empty. You have {} gold.", p.gold)
                } else {
                    let items: Vec<String> = p.sack.iter().map(|i| i.name()).collect();
                    format!("Sack: {}. You have {} gold.", items.join(", "), p.gold)
                });
                lines.push(match p.relic {
                    Some(relic) => format!("Worn relic: {} — {}.", relic.name(), relic.description()),
                    None => "Worn relic: none. You may wear one relic at a time.".into(),
                });
                for relic in Relic::ALL {
                    if p.sack.contains(&Item::Relic(relic)) {
                        lines.push(format!("{} — {}.", relic.name(), relic.description()));
                    }
                }
                for line in lines {
                    self.say(Tone::Info, line);
                }
            }
            Instant::List => {
                if self.zone() != Zone::Shop {
                    self.say(Tone::System, "You are not in the shop.");
                    return;
                }
                let wares: Vec<String> = SHOP
                    .iter()
                    .map(|(item, price)| match item {
                        Item::Arrows(n) => format!("{n} arrows {price}"),
                        other => format!("{} {price}", other.name()),
                    })
                    .collect();
                self.say(Tone::Info, format!("\"Have a look,\" says the outfitter. {}.", wares.join(", ")));
            }
            Instant::Skills => {
                for practice in Practice::ALL {
                    let skill = self.player.practice.get(practice);
                    self.say(
                        Tone::Info,
                        format!("{}: {} — {}", practice.name(), skill.rank().name(), self.practice_benefit(practice)),
                    );
                }
            }
            Instant::Look => {
                self.discover_wilderness();
                let place = match self.zone() {
                    Zone::Shop => "the outfitter's shop".to_owned(),
                    Zone::Temple => "the temple".to_owned(),
                    Zone::Open => self.level().name_at(self.player.pos).to_owned(),
                };
                let underfoot = match self.level().tile(self.player.pos) {
                    Tile::StairsDown => " Stairs lead down.",
                    Tile::StairsUp => " Stairs lead up.",
                    _ => "",
                };
                self.say(Tone::Info, format!("You are in {place}.{underfoot}"));
                self.town_directions();
                self.landmark_directions();
                if let Some(pile) = self.pile_at(self.player.pos) {
                    let items: Vec<String> = pile.items.iter().map(|i| i.name()).collect();
                    self.say(Tone::Info, format!("On the ground: {}.", items.join(", ")));
                }
                let sighted: Vec<String> = self
                    .sighted()
                    .iter()
                    .map(|s| {
                        let (dx, dy) = (s.creature.pos.0 - self.player.pos.0, s.creature.pos.1 - self.player.pos.1);
                        let way = Dir::from_delta(dx, dy).map_or("", Dir::short);
                        format!("{} {} ({} {way})", s.label, s.creature.stats().name, distance((0, 0), (dx, dy)))
                    })
                    .collect();
                if !sighted.is_empty() {
                    self.say(Tone::Info, format!("You see: {}.", sighted.join(", ")));
                }
            }
        }
    }

    fn practice_benefit(&self, practice: Practice) -> String {
        let skill = self.player.practice.get(practice);
        let steps = rank_steps(skill.rank());
        match practice {
            Practice::Dagger | Practice::Blades | Practice::Blunt | Practice::Staff | Practice::Archery => {
                format!("+{steps} accuracy, +{} damage", skill.bonus())
            }
            Practice::Evocation => format!("Spark: +{steps} accuracy, +{} damage", skill.bonus()),
            Practice::Warding => format!("ward: +{steps} protection, +{} rounds", 5 * steps),
            Practice::Restoration => format!("Heal: +{} health restored", 2 * steps),
            Practice::Binding => format!("Stun: +{steps} rounds"),
            Practice::Stealth => format!("+{}% hiding chance, +{} opening damage", 5 * steps, skill.bonus()),
            Practice::Vitality => format!("+{} maximum Health (currently {})", 4 * steps, self.player.max_hp),
            Practice::Focus => {
                format!("maximum Focus: {} ({} from practice)", self.player.max_focus, skill.focus_capacity())
            }
        }
    }

    // ----- The round -----

    /// Resolves one round: your command if any, then every creature, ward decay, and new arrivals.
    pub fn round(&mut self, command: Option<Command>) {
        if self.phase != Phase::Play {
            return;
        }
        self.footsteps.clear();
        let tracking: Vec<u32> =
            self.creatures.iter().filter(|c| c.hunting && c.last_seen.is_some()).map(|c| c.id).collect();
        let start = self.player.pos;
        let began_hidden = self.player.hidden;
        let hide_action = command.as_ref().is_some_and(|c| matches!(c, Command::Hide));
        let stealth_action = command.as_ref().is_some_and(|c| matches!(c, Command::Move(_) | Command::Hide));
        self.round += 1;
        match command {
            Some(command) => self.perform(command),
            None => self.stand_ready(),
        }
        if self.player.at == LevelId::Surface {
            self.towns.ensure(self.seed, self.terrain, self.player.pos);
        }
        self.bind_temple();
        self.enforce_light();
        self.populate_surface();
        self.creatures_act();
        if self.player.hp <= 0 {
            self.die();
        } else {
            if began_hidden && self.player.pos != start && self.player.hidden {
                self.learn_unseen_movement();
            }
            let moved = stealth_action && self.player.pos != start;
            let entered_hiding = hide_action && !began_hidden && self.player.hidden;
            let hide_broke_tracking =
                hide_action && tracking.iter().any(|id| self.creatures.iter().any(|c| c.id == *id && !c.hunting));
            if moved || entered_hiding || hide_broke_tracking {
                self.learn_escapes(&tracking);
            }
        }
        self.recover();
        self.respawn();
        self.update_seen();
    }

    fn perform(&mut self, command: Command) {
        match command {
            Command::Move(steps) => self.walk(&steps),
            Command::Attack(target) => self.attack(target),
            Command::Nock => self.nock(),
            Command::Shoot(target) => self.shoot(target),
            Command::Chant(spell) => self.chant(spell),
            Command::Cast(target) => self.cast(target),
            Command::Hide => self.hide(),
            Command::Get(what) => self.get(&what),
            Command::Drop(what) => self.drop_item(&what),
            Command::Wield(what) => self.wield(&what),
            Command::Wear(what) => self.wear(&what),
            Command::Remove(what) => self.remove_relic(&what),
            Command::Quaff => self.quaff(),
            Command::Use(what) => self.use_herb(&what),
            Command::Rest => self.rest(),
            Command::Down => self.stairs(true),
            Command::Up => self.stairs(false),
            Command::Buy(what) => self.buy(&what),
            Command::Sell(what) => self.sell(&what),
            Command::Heal => self.temple_heal(),
            Command::Wait => self.stand_ready(),
        }
    }

    /// Standing ready, you strike back at whatever is hunting you from the next cell.
    fn stand_ready(&mut self) {
        let me = self.player.pos;
        let attacker = self
            .sighted()
            .into_iter()
            .find(|s| s.creature.hunting && distance(s.creature.pos, me) <= 1)
            .map(|s| s.label);
        if let Some(label) = attacker {
            self.attack(Some(Target::Label(label)));
        }
    }

    fn hunter_adjacent(&self) -> Option<&Creature> {
        self.creatures.iter().find(|c| c.at == self.player.at && c.hunting && distance(c.pos, self.player.pos) <= 1)
    }

    fn walk(&mut self, steps: &[Dir]) {
        let steps = &steps[..steps.len().min(MAX_STEPS)];
        for (i, dir) in steps.iter().enumerate() {
            let (dx, dy) = dir.delta();
            let next = (self.player.pos.0 + dx, self.player.pos.1 + dy);
            if let Some(c) = self.creature_at(next) {
                let name = c.stats().name;
                self.say(Tone::Info, format!("A {name} is in the way."));
                return;
            }
            if !self.level().tile(next).walkable() {
                self.say(Tone::Info, "You can't go that way.");
                return;
            }
            self.player.pos = next;
            self.bind_temple();
            if self.player.hidden && !self.rng.chance(70 + modifier(self.player.dex) * 5) {
                self.player.hidden = false;
                self.say(Tone::Info, "You step out of the shadows.");
            }
            self.enforce_light();
            if self.player.at == LevelId::Surface {
                self.towns.ensure(self.seed, self.terrain, next);
                self.discover_wilderness();
            }
            self.describe_landmark();
            if self.footsteps.len() < MAX_STEPS {
                self.footsteps.push(Footstep { at: self.player.at, pos: next, quiet: self.player.hidden });
            }
            if let Some(pile) = self.pile_at(next) {
                let items: Vec<String> = pile.items.iter().map(|i| i.name()).collect();
                self.say(Tone::Info, format!("You see here: {}.", items.join(", ")));
            }
            if i + 1 < steps.len()
                && let Some(c) = self.hunter_adjacent()
            {
                let name = c.stats().name;
                self.say(Tone::Info, format!("You stop: a {name} is beside you."));
                return;
            }
        }
    }

    fn town_directions(&mut self) {
        if let Some(town) = self.level().town_at(self.player.pos) {
            for (service, at) in
                [("Outfitter", town.shop()), ("Temple", town.respawn()), ("Dungeon stairs", town.stairs())]
            {
                let me = self.player.pos;
                let way = Dir::from_delta(at.0 - me.0, at.1 - me.1).map_or("here", Dir::short);
                self.say(Tone::Info, format!("{service}: {} {way}.", distance(me, at)));
            }
        }
    }

    fn describe_landmark(&mut self) {
        if let Some(site) = self.landmark_at(self.player.pos) {
            let detail = if site.opened { "The cache is empty." } else { site.kind.description() };
            let instruction = if site.opened { "" } else { " Use get all to open it." };
            self.say(Tone::Info, format!("{}: {detail}{instruction}", site.kind.name()));
        }
        if let Some(patch) = self.herb_at(self.player.pos).filter(|patch| !patch.picked) {
            let herb = patch.herb;
            self.say(
                Tone::Info,
                format!("{} grows here; {}. Use get all to gather it.", herb.name(), herb.description()),
            );
        }
    }

    fn landmark_directions(&mut self) {
        self.describe_landmark();
        let me = self.player.pos;
        let lines: Vec<String> = self
            .landmarks
            .iter()
            .filter(|site| self.player.at == LevelId::Surface && site.pos != me && self.sees(site.pos))
            .map(|site| {
                let way = Dir::from_delta(site.pos.0 - me.0, site.pos.1 - me.1).map_or("here", Dir::short);
                format!(
                    "{}: {} {way} ({}).",
                    site.kind.name(),
                    distance(me, site.pos),
                    if site.opened { "empty cache" } else { "unopened cache" }
                )
            })
            .collect();
        for line in lines {
            self.say(Tone::Info, line);
        }
        let lines: Vec<String> = self
            .herbs
            .iter()
            .filter(|patch| {
                self.player.at == LevelId::Surface && !patch.picked && patch.pos != me && self.sees(patch.pos)
            })
            .map(|patch| {
                let way = Dir::from_delta(patch.pos.0 - me.0, patch.pos.1 - me.1).map_or("here", Dir::short);
                format!("{}: {} {way} ({}).", patch.herb.name(), distance(me, patch.pos), patch.herb.description())
            })
            .collect();
        for line in lines {
            self.say(Tone::Info, line);
        }
    }

    fn bind_temple(&mut self) {
        if self.player.at == LevelId::Surface
            && self.zone() == Zone::Temple
            && let Some(town) = self.level().town_at(self.player.pos)
            && self.revival != town.respawn()
        {
            self.revival = town.respawn();
            self.say(Tone::System, format!("If you fall, you will wake in {}'s temple.", town.name()));
        }
    }

    /// Resolves `target` among creatures in sight; `None` picks the nearest.
    fn find_target(&mut self, target: Option<Target>) -> Option<usize> {
        let sighted = self.sighted();
        let found = match &target {
            None => sighted.first(),
            Some(Target::Label(l)) => sighted.iter().find(|s| s.label == *l),
            Some(Target::Named { nth, name }) => sighted
                .iter()
                .filter(|s| {
                    let typed: Vec<&str> = name.split_whitespace().collect();
                    let name = s.creature.stats().name.to_lowercase();
                    let words: Vec<&str> = name.split(' ').collect();
                    typed.iter().all(|t| words.iter().any(|w| w.starts_with(t)))
                })
                .nth(nth - 1),
        }
        .map(|s| s.creature.id);
        let Some(id) = found else {
            let what = match target {
                None => "You see nothing to fight.".to_owned(),
                Some(Target::Label(l)) => format!("No creature is marked {l}."),
                Some(Target::Named { name, .. }) => format!("You see no {name} here."),
            };
            self.say(Tone::Info, what);
            return None;
        };
        self.creatures.iter().position(|c| c.id == id)
    }

    fn attack(&mut self, target: Option<Target>) {
        let target = match target {
            Some(t) => Some(t),
            None => {
                // A bare attack means whatever stands beside you.
                let me = self.player.pos;
                let beside = self.sighted().into_iter().find(|s| distance(s.creature.pos, me) <= 1).map(|s| s.label);
                beside.map(Target::Label)
            }
        };
        let Some(index) = self.find_target(target) else { return };
        let name = self.creatures[index].stats().name;
        if distance(self.creatures[index].pos, self.player.pos) > 1 {
            self.say(Tone::Info, format!("The {name} is out of reach."));
            return;
        }
        self.player.nocked = false;
        let ambush = std::mem::take(&mut self.player.hidden);
        if ambush {
            self.creatures[index].revealed_by_attack = true;
        }
        let opening = ambush && !self.creatures[index].hunting;
        let practice = Practice::weapon(self.player.weapon);
        let weapon_skill = self.player.practice.get(practice);
        let stealth_skill = self.player.practice.get(Practice::Stealth);
        let ac = self.creatures[index].stats().ac;
        let roll = self.rng.d20() + self.player.melee_bonus() + if ambush { 4 } else { 0 };
        if roll < ac && roll - self.player.melee_bonus() != 20 {
            self.say(Tone::Miss, format!("You miss the {name}."));
            self.provoke(index);
            return;
        }
        let (lo, hi) = self.player.weapon.damage();
        let mut damage = (self.rng.range(lo, hi) + modifier(self.player.str) + weapon_skill.bonus()).max(1);
        if ambush {
            if opening {
                damage += stealth_skill.bonus();
            }
            damage *= 3;
            self.say(Tone::Hit, format!("You strike the {name} from the shadows for {damage}!"));
        } else {
            self.say(Tone::Hit, format!("You hit the {name} for {damage}."));
        }
        if opening {
            self.teach_stealth(index, 3);
        }
        self.wound(index, damage, practice, 0);
    }

    fn nock(&mut self) {
        let p = &self.player;
        let refusal = match (p.bow, p.arrows, p.nocked) {
            (None, _, _) => Some("You have no bow."),
            (_, _, true) => Some("An arrow is already nocked."),
            (_, 0, _) => Some("Your quiver is empty."),
            _ => None,
        };
        if let Some(refusal) = refusal {
            self.say(Tone::Info, refusal);
            return;
        }
        self.player.nocked = true;
        self.say(Tone::Info, "You nock an arrow.");
    }

    /// Checks range and a clear line to the creature at `index`.
    fn in_line(&mut self, index: usize, range: i32) -> bool {
        let name = self.creatures[index].stats().name;
        let to = self.creatures[index].pos;
        if distance(self.player.pos, to) > range {
            self.say(Tone::Info, format!("The {name} is out of range."));
            return false;
        }
        let blocked = crate::map::line(self.player.pos, to)
            .into_iter()
            .skip(1)
            .take_while(|&p| p != to)
            .any(|p| self.creatures.iter().any(|c| c.at == self.player.at && c.pos == p));
        if blocked {
            self.say(Tone::Info, format!("Something stands between you and the {name}."));
            return false;
        }
        true
    }

    fn shoot(&mut self, target: Option<Target>) {
        let Some(bow) = self.player.bow else {
            self.say(Tone::Info, "You have no bow.");
            return;
        };
        if !self.player.nocked {
            self.say(Tone::Info, "You must nock an arrow first.");
            return;
        }
        let Some(index) = self.find_target(target) else { return };
        if !self.in_line(index, bow.range()) {
            return;
        }
        self.player.nocked = false;
        self.player.arrows -= 1;
        let ambush = std::mem::take(&mut self.player.hidden);
        if ambush {
            self.creatures[index].revealed_by_attack = true;
        }
        let opening = ambush && !self.creatures[index].hunting;
        let archery = self.player.practice.get(Practice::Archery);
        let stealth = self.player.practice.get(Practice::Stealth);
        let stats = self.creatures[index].stats();
        let to = self.creatures[index].pos;
        let close = distance(self.player.pos, to) <= 1;
        let natural = self.rng.d20();
        let roll = natural + self.player.missile_bonus() + if ambush { 4 } else { 0 } - if close { 4 } else { 0 };
        if roll < stats.ac && natural != 20 {
            self.say(Tone::Miss, format!("Your arrow misses the {}.", stats.name));
            if self.rng.chance(75) {
                self.drop_at(self.player.at, to, Item::Arrows(1));
            }
            self.provoke(index);
            return;
        }
        let (lo, hi) = bow.damage();
        let mut damage = (self.rng.range(lo, hi) + modifier(self.player.dex).max(0) + archery.bonus()).max(1);
        if ambush {
            if opening {
                damage += stealth.bonus();
            }
            damage *= 2;
        }
        if stats.arrow_proof {
            damage = (damage / 2).max(1);
        }
        let how = if ambush { "from hiding " } else { "" };
        self.say(Tone::Hit, format!("Your arrow {how}strikes the {} for {damage}!", stats.name));
        if self.rng.chance(50) {
            self.drop_at(self.player.at, to, Item::Arrows(1));
        }
        if opening {
            self.teach_stealth(index, 3);
        }
        self.wound(index, damage, Practice::Archery, 0);
    }

    fn chant(&mut self, spell: Spell) {
        self.player.chanted = Some(spell);
        self.say(Tone::Info, format!("You chant the words of {}.", spell.name()));
    }

    fn cast(&mut self, target: Option<Target>) {
        let Some(spell) = self.player.chanted else {
            self.say(Tone::Info, "You have no spell chanted.");
            return;
        };
        let metal_penalty = self.player.armor.is_some_and(Armor::metal) && spell != Spell::Shield;
        let cost = spell.cost() + if metal_penalty { 2 } else { 0 };
        if self.player.focus < cost {
            self.say(Tone::Info, format!("You lack the Focus for {} ({cost} needed).", spell.name()));
            return;
        }
        if !spell.aimed() {
            self.player.focus -= cost;
            self.player.chanted = None;
            if spell == Spell::Heal {
                let skill = self.player.practice.get(Practice::Restoration);
                let before = self.player.hp;
                let healing = (self.rng.range(4, 12) + 2 * rank_steps(skill.rank())
                    - if self.player.relic == Some(Relic::EmberCharm) { 3 } else { 0 })
                .max(1);
                self.player.hp = (self.player.hp + healing).min(self.player.max_hp);
                let healed = self.player.hp - before;
                self.say(Tone::Gain, format!("Warmth knits your wounds. (+{healed} health)"));
                let injuries = self.consume_injuries(healed);
                if healed > 0 {
                    self.teach_heal(cost, injuries);
                }
            } else {
                let skill = self.player.practice.get(Practice::Warding);
                self.player.shield = 20 + 5 * rank_steps(skill.rank()) as u32;
                self.player.shield_focus_credit = cost as u32;
                self.say(Tone::Gain, "A shimmering ward settles around you.");
            }
            return;
        }
        let Some(index) = self.find_target(target) else { return };
        if !self.in_line(index, SIGHT) {
            return;
        }
        self.player.focus -= cost;
        self.player.chanted = None;
        let revealed = std::mem::take(&mut self.player.hidden);
        if revealed {
            self.creatures[index].revealed_by_attack = true;
        }
        let stats = self.creatures[index].stats();
        let natural = self.rng.d20();
        let discipline = Practice::spell(spell);
        let rank_accuracy = rank_steps(self.player.practice.get(discipline).rank());
        let roll = natural + modifier(self.player.int) + 2 + rank_accuracy;
        if roll < stats.ac && natural != 20 {
            let text = match spell {
                Spell::Stun => format!("The {} shrugs off your stun.", stats.name),
                _ => format!("Your spark crackles past the {}.", stats.name),
            };
            self.say(Tone::Miss, text);
            self.provoke(index);
            return;
        }
        if spell == Spell::Stun {
            // Larger creatures resist being held; Binding practice adds a few rounds.
            let base = if self.creatures[index].kind == Kind::Troll { 1 } else { self.rng.range(2, 3) as u32 };
            let rounds = base + rank_accuracy as u32;
            let prior = self.creatures[index].stunned;
            self.creatures[index].stunned = self.creatures[index].stunned.max(rounds);
            self.say(Tone::Hit, format!("The {} reels, stunned!", stats.name));
            self.provoke(index);
            let gained = self.creatures[index].stunned.saturating_sub(prior);
            if gained > 0 && self.teach_utility(index, Practice::Binding, gained) {
                let threat = self.threat(index, Practice::Focus);
                if threat >= 10 {
                    self.learn_practice(Practice::Focus, (cost as u32).saturating_mul(threat), threat);
                }
            }
            return;
        }
        let skill = self.player.practice.get(Practice::Evocation);
        let damage = (self.rng.range(2, 8)
            + modifier(self.player.int).max(0)
            + skill.bonus()
            + if self.player.relic == Some(Relic::EmberCharm) { 2 } else { 0 })
        .max(1);
        self.say(Tone::Hit, format!("A spark leaps from your hand and burns the {} for {damage}!", stats.name));
        self.wound(index, damage, Practice::Evocation, cost);
    }

    fn hide(&mut self) {
        if self.illuminated(self.player.pos) {
            self.say(Tone::Info, "The glow drives away the shadows; there is nowhere to hide.");
            return;
        }
        if let Some(c) = self.hunter_adjacent() {
            let name = c.stats().name;
            self.say(Tone::Info, format!("The {name} is watching you too closely."));
            return;
        }
        let watchers = self.creatures.iter().filter(|c| c.at == self.player.at && c.hunting).count() as i32;
        if self.rng.chance(self.player.hiding_chance(watchers)) {
            self.player.hidden = true;
            for c in self.creatures.iter_mut().filter(|c| c.at == self.player.at) {
                c.hunting = false;
                c.last_seen = None;
                c.lost = 0;
                c.listening = false;
                c.revealed_by_attack = false;
            }
            self.say(Tone::Info, "You melt into the shadows.");
        } else {
            self.say(Tone::Info, "You fail to find any cover.");
        }
    }

    /// Hurts a creature and spends its shared finite allowance for useful damage.
    fn wound(&mut self, index: usize, damage: i32, discipline: Practice, focus_spent: i32) {
        let threat = self.threat(index, discipline);
        let useful = self.creatures[index].spend_damage(damage);
        self.creatures[index].hp -= damage;
        if useful > 0 && threat >= 10 {
            let evidence = useful.saturating_mul(threat);
            self.learn_practice(discipline, evidence, threat);
            if focus_spent > 0 {
                self.learn_practice(Practice::Focus, (focus_spent as u32).saturating_mul(threat), threat);
            }
        }
        if self.creatures[index].hp > 0 {
            self.provoke(index);
            return;
        }
        let dead = self.creatures.remove(index);
        self.prune_injury_sources();
        let stats = dead.stats();
        self.say(Tone::Kill, format!("The {} dies! (+{} experience)", stats.name, stats.xp));
        let den = dead.spawn.is_some_and(|spawn| self.spawns[spawn].den);
        if let Some(spawn) = dead.spawn {
            self.spawns[spawn].occupant = None;
            self.spawns[spawn].ready = self.round + if den { DEN_RESPAWN } else { RESPAWN };
        }
        let gold = self.rng.range(stats.gold.0, stats.gold.1);
        if gold > 0 {
            self.drop_at(dead.at, dead.pos, Item::Gold(gold as u32));
        }
        if den {
            self.say(Tone::Gain, "Something glints among the bones of its den.");
        }
        for item in self.loot(dead.kind, den, stats.xp) {
            self.drop_at(dead.at, dead.pos, item);
        }
        self.gain_xp(stats.xp);
    }

    fn learn_practice(&mut self, discipline: Practice, evidence: u32, threat: u32) {
        let Some(rank) = self.player.practice.learn(discipline, evidence, threat) else { return };
        match discipline {
            Practice::Vitality => {
                self.player.max_hp = self.player.baseline_max_hp() + 4 * rank_steps(rank);
            }
            Practice::Focus => {
                self.player.refresh_focus_capacity();
            }
            _ => {}
        }
        self.say(Tone::Gain, format!("Your {} practice is now {}.", discipline.name(), rank.name()));
    }

    fn teach_utility(&mut self, index: usize, discipline: Practice, effect: u32) -> bool {
        let threat = self.threat(index, discipline);
        self.teach_utility_at_threat(index, discipline, effect, threat)
    }

    fn teach_utility_at_threat(&mut self, index: usize, discipline: Practice, effect: u32, threat: u32) -> bool {
        if threat < 10 || effect == 0 || !self.creatures[index].spend_utility_lesson() {
            return false;
        }
        self.learn_practice(discipline, effect.saturating_mul(threat), threat);
        true
    }

    fn teach_heal(&mut self, cost: i32, injuries: Vec<HealedInjury>) {
        let mut focus_threat = 0;
        for injury in injuries {
            if let Some(index) = self
                .creatures
                .iter()
                .position(|creature| creature.id == injury.creature_id && creature.at == self.player.at)
                && self.encounter_active(index)
                && self.teach_utility_at_threat(index, Practice::Restoration, injury.amount, injury.threat)
            {
                focus_threat = focus_threat.max(injury.threat);
            }
        }
        if cost > 0 && focus_threat >= 10 {
            self.learn_practice(Practice::Focus, (cost as u32).saturating_mul(focus_threat), focus_threat);
        }
    }

    fn consume_injuries(&mut self, amount: i32) -> Vec<HealedInjury> {
        let healed = self.player.consume_injuries(amount);
        self.prune_injury_sources();
        healed
    }

    fn prune_injury_sources(&mut self) {
        self.player.injuries.retain(|source| {
            source.remaining > 0 || self.creatures.iter().any(|creature| creature.id == source.creature_id)
        });
    }

    fn teach_stealth(&mut self, index: usize, lessons: u8) {
        let threat = self.threat(index, Practice::Stealth);
        if threat < 10 {
            return;
        }
        let earned = self.creatures[index].spend_stealth(lessons);
        if earned > 0 {
            self.learn_practice(Practice::Stealth, earned.saturating_mul(threat).saturating_mul(2), threat);
        }
    }

    fn encounter_active(&self, index: usize) -> bool {
        let creature = &self.creatures[index];
        if !creature.teaches_anything() {
            return false;
        }
        if creature.hunting {
            return true;
        }
        if self.player.hidden || self.zone() != Zone::Open {
            return false;
        }
        self.creature_can_sense(index, self.player.pos)
    }

    fn creature_can_sense(&self, index: usize, target: Pos) -> bool {
        self.creature_can_see(index, target)
            || self.footsteps.iter().any(|step| step.pos == target && self.can_hear_footstep(index, step).is_some())
    }

    fn creature_can_see(&self, index: usize, target: Pos) -> bool {
        let creature = &self.creatures[index];
        let leash = if creature.at == LevelId::Surface { WILD_LEASH } else { DEEP_LEASH };
        if creature.at != self.player.at
            || distance(target, creature.home) > leash + 1
            || (creature.phenotype().is_some() && self.level().safe(target))
        {
            return false;
        }
        let notice = creature.phenotype().map_or(NOTICE, |p| p.sense as i32);
        distance(creature.pos, target) <= notice && self.level().sees(creature.pos, target, notice)
    }

    /// The traversable path to a heard step, bounded to its audible range or the loud-footstep limit.
    fn tremor_route(&self, index: usize, goal: Pos, max_distance: usize) -> Option<(usize, Option<Pos>)> {
        let creature = &self.creatures[index];
        if creature.at != self.player.at
            || !matches!(creature.at, LevelId::Deep(..))
            || self.level().safe(goal)
            || self.level().zone(goal) != Zone::Open
            || !self.level().tile(goal).walkable()
        {
            return None;
        }
        let leash = if creature.at == LevelId::Surface { WILD_LEASH } else { DEEP_LEASH };
        let leash_limit = leash.max(distance(creature.pos, creature.home));
        let goal_occupied = goal == self.player.pos
            || self
                .creatures
                .iter()
                .any(|other| other.id != creature.id && other.at == self.player.at && other.pos == goal);
        let mut queue = VecDeque::from([(creature.pos, 0_usize, None)]);
        let mut visited = BTreeSet::from([creature.pos]);
        while let Some((pos, steps, first)) = queue.pop_front() {
            if pos == goal {
                return Some((steps, first));
            }
            if goal_occupied && distance(pos, goal) <= 1 {
                return Some((steps + 1, first));
            }
            if steps >= max_distance {
                continue;
            }
            for dir in [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw] {
                let (dx, dy) = dir.delta();
                let next = (pos.0 + dx, pos.1 + dy);
                if distance(next, creature.home) > leash_limit
                    || !self.open_for_creature(index, next)
                    || !visited.insert(next)
                {
                    continue;
                }
                let first = first.or(Some(next));
                queue.push_back((next, steps + 1, first));
            }
        }
        None
    }

    fn can_hear_footstep(&self, index: usize, step: &Footstep) -> Option<usize> {
        let creature = &self.creatures[index];
        let range = if step.quiet { TREMOR_QUIET_RANGE } else { TREMOR_LOUD_RANGE };
        if step.at != creature.at
            || !creature.cave_traits().tremorsense
            || distance(step.pos, creature.home) > DEEP_LEASH + 1
        {
            return None;
        }
        self.tremor_route(index, step.pos, range).and_then(|(distance, _)| (distance <= range).then_some(distance))
    }

    /// The latest real step this tremorsensing creature can hear this round.
    fn heard_footstep(&self, index: usize) -> Option<Pos> {
        self.footsteps.iter().rev().find_map(|step| self.can_hear_footstep(index, step).map(|_| step.pos))
    }

    fn learn_unseen_movement(&mut self) {
        let candidates: Vec<u32> = self
            .creatures
            .iter()
            .enumerate()
            .filter(|(i, c)| {
                c.at == self.player.at
                    && !c.hunting
                    && !c.phenotype().is_some_and(|p| p.temperament == Temperament::Skittish)
                    && c.teaches_anything()
                    && self.creature_can_sense(*i, self.player.pos)
                    && self.heard_footstep(*i).is_none()
            })
            .map(|(_, c)| c.id)
            .collect();
        for id in candidates {
            if let Some(index) = self.creatures.iter().position(|c| c.id == id) {
                self.teach_stealth(index, 1);
            }
        }
    }

    fn learn_escapes(&mut self, tracking: &[u32]) {
        for id in tracking {
            if let Some(index) = self
                .creatures
                .iter()
                .position(|c| c.id == *id && !c.hunting && c.teaches_anything() && !c.revealed_by_attack)
            {
                self.teach_stealth(index, 3);
            }
        }
    }

    /// Threat is the approximate share of unskilled full Health a creature deals during a fair fight.
    /// Injury, Focus, earned practice, and adventuring rank do not enter the calculation.
    fn threat(&self, index: usize, discipline: Practice) -> u32 {
        let creature = &self.creatures[index];
        if !creature.teaches_anything() {
            return 0;
        }
        let stats = creature.stats();
        let (damage, accuracy) = match discipline {
            Practice::Dagger => (Weapon::Dagger.damage(), modifier(self.player.str) + Weapon::Dagger.accuracy()),
            Practice::Blades => {
                (Weapon::ShortSword.damage(), modifier(self.player.str) + Weapon::ShortSword.accuracy())
            }
            Practice::Blunt => (Weapon::Mace.damage(), modifier(self.player.str) + Weapon::Mace.accuracy()),
            Practice::Staff => (Weapon::Staff.damage(), modifier(self.player.str) + Weapon::Staff.accuracy()),
            Practice::Archery => (Bow::Shortbow.damage(), modifier(self.player.dex)),
            Practice::Evocation | Practice::Warding | Practice::Restoration | Practice::Binding | Practice::Focus => {
                ((2, 8), modifier(self.player.int) + 2)
            }
            Practice::Stealth => {
                let weapon = self.player.weapon;
                (weapon.damage(), modifier(self.player.str) + weapon.accuracy())
            }
            Practice::Vitality => {
                let weapon = self.player.weapon;
                (weapon.damage(), modifier(self.player.str) + weapon.accuracy())
            }
        };
        let output = ((damage.0 + damage.1) / 2
            + match discipline {
                Practice::Archery => modifier(self.player.dex).max(0),
                Practice::Evocation
                | Practice::Warding
                | Practice::Restoration
                | Practice::Binding
                | Practice::Focus => modifier(self.player.int).max(0),
                _ => modifier(self.player.str),
            })
        .max(1);
        let player_hit = (21 + accuracy - stats.ac).clamp(1, 20) as i128 * 5;
        let incoming_damage = stats.shot.map_or(stats.damage, |(_, shot)| {
            if (shot.0 + shot.1) > (stats.damage.0 + stats.damage.1) { shot } else { stats.damage }
        });
        let enemy_output = ((incoming_damage.0 + incoming_damage.1) / 2).max(1);
        let enemy_hit = (21 + stats.hit - self.player.ac()).clamp(1, 20) as i128 * 5;
        let speed = creature.speed().clamp(0, blah_core::ACTION_COST * blah_core::MAX_ACTIONS as i32) as i128;
        let numerator = 100_i128 * i128::from(stats.hp.max(0)) * i128::from(enemy_output) * enemy_hit * speed;
        let denominator = i128::from(output)
            * player_hit
            * i128::from(blah_core::ACTION_COST)
            * i128::from(self.player.baseline_max_hp());
        if denominator <= 0 { 0 } else { (numerator / denominator).clamp(0, 200) as u32 }
    }
    /// What a kill leaves besides its coins. Evolved life drops what it has gathered or swallowed: likelier,
    /// and better, the more experience it is worth; a den keeper sits on a hoard.
    fn loot(&mut self, kind: Kind, den: bool, xp: u32) -> Vec<Item> {
        if den {
            let mut hoard = treasure(xp * 8, 100, &mut self.rng);
            hoard.push(Item::Potion);
            return hoard;
        }
        if kind == Kind::Wild {
            let chance = (5 + xp as i32 / 2).min(60);
            return treasure(xp * 3, chance, &mut self.rng)
                .into_iter()
                .filter(|i| !matches!(i, Item::Gold(_)))
                .collect();
        }
        if kind == Kind::Troll {
            return vec![Item::Ring, Item::Potion];
        }
        let mut items = Vec::new();
        if kind == Kind::GoblinArcher && self.rng.chance(40) {
            items.push(Item::Arrows(self.rng.range(3, 6) as u32));
        }
        if kind != Kind::Rat && kind != Kind::Wolf && self.rng.chance(15) {
            items.push(match self.rng.below(10) {
                0..5 => Item::Potion,
                5..8 => Item::Arrows(self.rng.range(4, 9) as u32),
                8 => Item::Weapon([Weapon::Mace, Weapon::LongSword, Weapon::ShortSword][self.rng.below(3) as usize]),
                _ => Item::Armor([Armor::Leather, Armor::Chainmail][self.rng.below(2) as usize]),
            });
        }
        items
    }

    fn provoke(&mut self, index: usize) {
        let c = &mut self.creatures[index];
        c.hunting = true;
        c.lost = 0;
        c.last_seen = Some(self.player.pos);
        c.listening = false;
    }

    fn gain_xp(&mut self, xp: u32) {
        self.player.xp = self.player.xp.saturating_add(xp);
        while let Some(needed) = self.player.next_level()
            && self.player.xp >= needed
        {
            self.player.level += 1;
            self.say(Tone::Gain, format!("You reach adventuring rank {}.", self.player.level));
        }
    }

    fn drop_at(&mut self, at: LevelId, pos: Pos, item: Item) {
        let index = match self.piles.iter().position(|p| p.at == at && p.pos == pos) {
            Some(i) => i,
            None => {
                self.piles.push(Pile { at, pos, items: Vec::new() });
                self.piles.len() - 1
            }
        };
        let items = &mut self.piles[index].items;
        let merged = items.iter_mut().find_map(|held| match (held, item) {
            (Item::Gold(a), Item::Gold(b)) | (Item::Arrows(a), Item::Arrows(b)) => {
                *a += b;
                Some(())
            }
            _ => None,
        });
        if merged.is_none() {
            items.push(item);
        }
    }

    fn get(&mut self, what: &str) {
        self.discover_wilderness();
        let (at, pos) = (self.player.at, self.player.pos);
        if what == "all" && self.landmark_at(pos).is_some_and(|site| !site.opened) {
            let index = self.landmarks.iter().position(|site| site.pos == pos).expect("site underfoot");
            let site = self.landmarks[index];
            self.landmarks[index].opened = true;
            for item in site.contents(self.seed) {
                self.drop_at(at, pos, item);
            }
            self.say(Tone::Gain, format!("You open the cache at the {}.", site.kind.name()));
        }
        let gathered = self.gather_herb(what);
        let Some(index) = self.piles.iter().position(|p| p.at == at && p.pos == pos) else {
            if !gathered {
                self.say(Tone::Info, "There is nothing here.");
            }
            return;
        };
        let wanted: Vec<usize> = if what == "all" {
            (0..self.piles[index].items.len()).collect()
        } else {
            self.piles[index].items.iter().position(|i| i.matches(what)).into_iter().collect()
        };
        if wanted.is_empty() {
            if !gathered {
                self.say(Tone::Info, format!("You see no {what} here."));
            }
            return;
        }
        let mut taken = Vec::new();
        let mut left = Vec::new();
        for (i, item) in std::mem::take(&mut self.piles[index].items).into_iter().enumerate() {
            let fits = matches!(item, Item::Gold(_) | Item::Arrows(_)) || self.player.sack.len() < SACK;
            if wanted.contains(&i) && fits {
                match item {
                    Item::Gold(n) => self.player.gold += n,
                    Item::Arrows(n) => self.player.arrows += n,
                    other => self.player.sack.push(other),
                }
                taken.push(item.name());
            } else {
                if wanted.contains(&i) {
                    self.say(Tone::Info, format!("Your sack is full; you leave the {}.", item.name()));
                }
                left.push(item);
            }
        }
        if left.is_empty() {
            self.piles.remove(index);
        } else {
            self.piles[index].items = left;
        }
        if !taken.is_empty() {
            self.say(Tone::Gain, format!("You pick up {}.", taken.join(", ")));
        }
    }

    fn drop_item(&mut self, what: &str) {
        let (at, pos) = (self.player.at, self.player.pos);
        let item = if Item::Gold(0).matches(what) && self.player.gold > 0 {
            Item::Gold(std::mem::take(&mut self.player.gold))
        } else if Item::Arrows(0).matches(what) && self.player.arrows > 0 {
            self.player.nocked = false;
            Item::Arrows(std::mem::take(&mut self.player.arrows))
        } else if let Some(i) = self.player.sack.iter().position(|i| i.matches(what)) {
            self.player.sack.remove(i)
        } else {
            self.say(Tone::Info, format!("You carry no {what} in your sack."));
            return;
        };
        self.drop_at(at, pos, item);
        self.say(Tone::Info, format!("You drop {}.", item.name()));
    }

    fn wield(&mut self, what: &str) {
        let Some(i) = self.player.sack.iter().position(|i| i.matches(what)) else {
            self.say(Tone::Info, format!("You carry no {what} in your sack."));
            return;
        };
        match self.player.sack[i] {
            Item::Weapon(w) => {
                self.player.sack[i] = Item::Weapon(self.player.weapon);
                self.player.weapon = w;
                self.say(Tone::Info, format!("You wield the {}.", w.name()));
            }
            Item::Bow(b) => {
                self.player.nocked = false;
                match self.player.bow.replace(b) {
                    Some(old) => self.player.sack[i] = Item::Bow(old),
                    None => _ = self.player.sack.remove(i),
                }
                self.say(Tone::Info, format!("You take up the {}.", b.name()));
            }
            other => self.say(Tone::Info, format!("You can't wield {}.", other.name())),
        }
    }

    fn wear(&mut self, what: &str) {
        let Some(i) = self.player.sack.iter().position(|i| i.matches(what)) else {
            self.say(Tone::Info, format!("You carry no {what} in your sack."));
            return;
        };
        match self.player.sack[i] {
            Item::Relic(relic) => {
                match self.player.relic.replace(relic) {
                    Some(old) => self.player.sack[i] = Item::Relic(old),
                    None => {
                        self.player.sack.remove(i);
                    }
                }
                self.player.refresh_focus_capacity();
                self.say(Tone::Gain, format!("You wear the {}. {}.", relic.name(), relic.description()));
            }
            Item::Armor(a) => {
                match self.player.armor.replace(a) {
                    Some(old) => self.player.sack[i] = Item::Armor(old),
                    None => _ = self.player.sack.remove(i),
                }
                self.say(Tone::Info, format!("You put on the {}.", a.name()));
            }
            Item::Ring if !self.player.ring => {
                self.player.sack.remove(i);
                self.player.ring = true;
                self.say(Tone::Gain, "You slip on the ring of warding. Blows seem to turn aside.");
            }
            other => self.say(Tone::Info, format!("You can't wear {}.", other.name())),
        }
    }

    fn remove_relic(&mut self, what: &str) {
        let Some(relic) = self.player.relic.filter(|&relic| Item::Relic(relic).matches(what)) else {
            self.say(Tone::Info, format!("You wear no {what}."));
            return;
        };
        if self.player.sack.len() >= SACK {
            self.say(Tone::Info, "Your sack is full; make room before removing your relic.");
            return;
        }
        self.player.relic = None;
        self.player.sack.push(Item::Relic(relic));
        self.player.refresh_focus_capacity();
        self.say(Tone::Info, format!("You remove the {}. Its benefit and cost end.", relic.name()));
    }

    fn quaff(&mut self) {
        let Some(i) = self.player.sack.iter().position(|&i| i == Item::Potion) else {
            self.say(Tone::Info, "You have no potion.");
            return;
        };
        self.player.sack.remove(i);
        let healed = self.rng.range(10, 16);
        self.player.hp = (self.player.hp + healed).min(self.player.max_hp);
        self.consume_injuries(healed);
        self.say(Tone::Gain, format!("You drink the potion. (+{healed} health)"));
    }

    fn gather_herb(&mut self, what: &str) -> bool {
        let Some(patch) = self.herb_at(self.player.pos).copied().filter(|patch| !patch.picked) else {
            return false;
        };
        let item = Item::Herb(patch.herb);
        if what != "all" && !item.matches(what) {
            return false;
        }
        if self.player.sack.len() >= SACK {
            self.say(Tone::Info, format!("Your sack is full; you leave the {} growing.", patch.herb.name()));
        } else {
            let index = self.herbs.iter().position(|known| known.pos == patch.pos).expect("patch underfoot");
            self.herbs[index].picked = true;
            self.player.sack.push(item);
            self.say(Tone::Gain, format!("You gather {}. It {}.", patch.herb.name(), patch.herb.description()));
        }
        true
    }

    fn use_herb(&mut self, what: &str) {
        let Some(index) = self.player.sack.iter().position(|item| item.matches(what)) else {
            self.say(Tone::Info, format!("You carry no {what} in your sack."));
            return;
        };
        let Item::Herb(herb) = self.player.sack[index] else {
            self.say(Tone::Info, "Use names a herb. Drink a healing potion with quaff.");
            return;
        };
        let (health, focus) = herb.recovery();
        let health = (self.player.max_hp - self.player.hp).clamp(0, health);
        let focus = (self.player.max_focus - self.player.focus).clamp(0, focus);
        if health == 0 && focus == 0 {
            self.say(Tone::Info, format!("The {} would not help you now; you keep it.", herb.name()));
            return;
        }
        self.player.sack.remove(index);
        self.player.hp += health;
        self.player.focus += focus;
        self.consume_injuries(health);
        self.say(Tone::Gain, format!("You use {}. (+{health} Health, +{focus} Focus)", herb.name()));
    }

    fn rest(&mut self) {
        if let Some(c) = self.creatures.iter().find(|c| c.at == self.player.at && c.hunting) {
            let name = c.stats().name;
            self.say(Tone::Info, format!("You can't rest with a {name} hunting you."));
            return;
        }
        let (health, focus) = {
            let p = &mut self.player;
            let health = (p.max_hp - p.hp).min(2);
            let focus = (p.max_focus - p.focus).min(6);
            p.hp += health;
            p.focus += focus;
            (health, focus)
        };
        self.consume_injuries(health);
        self.say(Tone::Info, format!("You rest a while. (+{health} health, +{focus} Focus)"));
    }

    fn stairs(&mut self, down: bool) {
        let level = self.level();
        let tile = level.tile(self.player.pos);
        let (destination, arrive) = match (down, tile, self.player.at, level.town_at(self.player.pos)) {
            (true, Tile::StairsDown, LevelId::Surface, Some(town)) => {
                (LevelId::Deep(town, 1), Deep::new(town, 1).stairs())
            }
            (true, Tile::StairsDown, LevelId::Deep(town, depth), _) => {
                (LevelId::Deep(town, depth + 1), Deep::new(town, depth + 1).stairs())
            }
            (false, Tile::StairsUp, LevelId::Deep(town, 1), _) => (LevelId::Surface, town.stairs()),
            (false, Tile::StairsUp, LevelId::Deep(town, depth), _) => {
                (LevelId::Deep(town, depth - 1), Deep::new(town, depth - 1).stairs_down())
            }
            _ => {
                let way = if down { "down" } else { "up" };
                self.say(Tone::Info, format!("There are no stairs {way} here."));
                return;
            }
        };
        if let LevelId::Deep(town, depth) = destination {
            self.open_deep(Deep::new(town, depth));
        }
        self.player.at = destination;
        self.player.pos = arrive;
        self.player.hidden = false;
        let text = match destination {
            LevelId::Deep(_, depth) if down && depth > 1 => {
                format!("You descend to depth {depth}. It is colder here, and something larger is listening.")
            }
            _ if down => {
                format!("You descend into {}. The air is cool, damp, and still.", self.level().name_at(arrive))
            }
            LevelId::Deep(_, depth) => format!("You climb back up to depth {depth}."),
            LevelId::Surface => format!("You climb back into the daylight of {}.", self.level().name_at(arrive)),
        };
        self.say(Tone::Info, text);
    }
    fn buy(&mut self, what: &str) {
        if self.zone() != Zone::Shop {
            self.say(Tone::Info, "There is no one here to buy from.");
            return;
        }
        let Some(&(item, price)) = SHOP.iter().find(|(i, _)| i.matches(what)) else {
            self.say(Tone::Info, format!("\"I don't sell {what},\" says the outfitter."));
            return;
        };
        if self.player.gold < price {
            self.say(
                Tone::Info,
                format!("\"That's {price} gold, and you have {},\" says the outfitter.", self.player.gold),
            );
            return;
        }
        match item {
            Item::Arrows(n) => self.player.arrows += n,
            Item::Bow(b) if self.player.bow.is_none() => self.player.bow = Some(b),
            other if self.player.sack.len() >= SACK => {
                self.say(Tone::Info, format!("Your sack has no room for the {}.", other.name()));
                return;
            }
            other => self.player.sack.push(other),
        }
        self.player.gold -= price;
        self.say(Tone::Gain, format!("You buy {} for {price} gold.", item.name()));
    }

    fn sell(&mut self, what: &str) {
        if self.zone() != Zone::Shop {
            self.say(Tone::Info, "There is no one here to sell to.");
            return;
        }
        let Some(i) = self.player.sack.iter().position(|i| i.matches(what)) else {
            self.say(Tone::Info, format!("You carry no {what} in your sack."));
            return;
        };
        let item = self.player.sack.remove(i);
        let price = sale_price(item);
        self.player.gold += price;
        self.say(Tone::Gain, format!("The outfitter pays {price} gold for the {}.", item.name()));
    }

    fn temple_heal(&mut self) {
        if self.zone() != Zone::Temple {
            self.say(Tone::Info, "Only the temple's priests can heal you.");
            return;
        }
        if self.player.hp == self.player.max_hp && self.player.focus == self.player.max_focus {
            self.say(Tone::Info, "\"You are whole already,\" the priest says.");
            return;
        }
        let (cost, health) = {
            let p = &mut self.player;
            let cost = 5.min(p.gold);
            p.gold -= cost;
            let health = p.max_hp - p.hp;
            p.hp = p.max_hp;
            p.focus = p.max_focus;
            (cost, health)
        };
        self.consume_injuries(health);
        self.say(Tone::Gain, format!("The priest lays hands on you. Health and Focus restored. ({cost} gold)"));
    }

    // ----- Creatures -----

    fn populate_surface(&mut self) {
        if self.player.at != LevelId::Surface || self.round < self.wild_ready {
            return;
        }
        let me = self.player.pos;
        self.creatures.retain(|c| c.at != LevelId::Surface || c.kind != Kind::Wild || distance(c.pos, me) <= 32);
        let level = self.level();
        if level.authored(me) {
            return;
        }
        let terrain = self.terrain;
        let productivity = terrain.biome(me).biome().productivity;
        let count = self.creatures.iter().filter(|c| c.kind == Kind::Wild).count();
        if count >= blah_core::capacity(productivity) {
            return;
        }
        let mut rng = blah_core::Rng::keyed(self.seed, b"islelife", &[self.round]);
        if count > 0 && !rng.chance(blah_core::spawn_chance(productivity)) {
            return;
        }
        for _ in 0..6 {
            let dx = rng.below(33) as i32 - 16;
            let dy = rng.below(33) as i32 - 16;
            let pos = (me.0 + dx, me.1 + dy);
            if distance(pos, me) <= SIGHT + 2
                || level.authored(pos)
                || self.creature_at(pos).is_some()
                || matches!(level.tile(pos), Tile::Rock | Tile::Wall)
            {
                continue;
            }
            let wild = self.life.draw(self.seed, terrain.world_pos(pos), &mut rng);
            if !wild.phenotype().access.allows(terrain.biome(pos).biome()) {
                continue;
            }
            let hp = wild.stats().hp;
            self.creatures.push(Creature {
                id: self.next_id,
                kind: Kind::Wild,
                wild: Some(wild),
                energy: 0,
                at: LevelId::Surface,
                pos,
                hp,
                home: pos,
                hunting: false,
                last_seen: None,
                lost: 0,
                listening: false,
                stunned: 0,
                learning_damage_left: hp,
                vitality_lessons_left: hp,
                stealth_lessons_left: 12,
                utility_lessons_left: 12,
                revealed_by_attack: false,
                spawn: None,
            });
            self.next_id += 1;
            self.wild_ready = self.round + 3;
            break;
        }
    }

    fn creatures_act(&mut self) {
        let ids: Vec<u32> = self.creatures.iter().filter(|c| c.at == self.player.at).map(|c| c.id).collect();
        for id in ids {
            if self.player.hp <= 0 {
                return;
            }
            let Some(index) = self.creatures.iter().position(|c| c.id == id) else { continue };
            if self.creatures[index].stunned > 0 {
                self.creatures[index].stunned -= 1;
                continue;
            }
            let actions = if let Some(p) = self.creatures[index].phenotype() {
                let c = &mut self.creatures[index];
                c.energy += p.stats.speed;
                let actions = (c.energy / blah_core::ACTION_COST).min(blah_core::MAX_ACTIONS as i32);
                c.energy -= actions * blah_core::ACTION_COST;
                actions
            } else {
                1
            };
            for _ in 0..actions {
                if self.player.hp <= 0 {
                    return;
                }
                self.creature_turn(index);
                if self.player.hp > 0 {
                    self.enforce_light();
                }
            }
        }
    }

    fn creature_turn(&mut self, index: usize) {
        let level = self.level();
        let me = self.player.pos;
        let pos = self.creatures[index].pos;
        let stats = self.creatures[index].stats();
        let phenotype = self.creatures[index].phenotype();
        let close = distance(pos, me) <= 1;
        let leash = if self.player.at == LevelId::Surface { WILD_LEASH } else { DEEP_LEASH };
        let home = self.creatures[index].home;
        let in_reach = distance(me, home) <= leash + 1 && (phenotype.is_none() || !level.safe(me));
        let in_sight = self.creature_can_see(index, me);
        let detects = in_sight && (!self.player.hidden || (close && self.rng.chance(30)));
        let heard_pos = self.heard_footstep(index);
        let provoked = self.creatures[index].hunting && !self.creatures[index].listening;
        let skittish = phenotype.is_some_and(|p| p.temperament == Temperament::Skittish);
        let territorial = phenotype.is_some_and(|p| p.temperament == Temperament::Territorial);
        let sound_notices = heard_pos.filter(|&source| !territorial || provoked || distance(source, home) <= 3);
        if skittish && !detects {
            // Losing the threat must not turn flight into a hunt back toward its last position.
            self.creatures[index].hunting = false;
            self.creatures[index].last_seen = None;
            self.creatures[index].listening = false;
        }
        if skittish && (detects || sound_notices.is_some()) {
            let threat = if detects { me } else { sound_notices.expect("a sound was heard") };
            let best = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw]
                .into_iter()
                .map(|d| (pos.0 + d.delta().0, pos.1 + d.delta().1))
                .filter(|&p| self.open_for_creature(index, p) && distance(p, home) <= WILD_LEASH)
                .max_by_key(|&p| distance(p, threat));
            if let Some(next) = best
                && distance(next, threat) > distance(pos, threat)
            {
                self.creatures[index].pos = next;
            } else if detects && provoked && close {
                self.creature_strike(index, stats.hit, stats.damage, None);
            }
            return;
        }
        let notices = detects && (!territorial || provoked || distance(me, home) <= 3);
        let listens = !notices && sound_notices.is_some();
        if !in_reach && !listens && self.creatures[index].hunting {
            let c = &mut self.creatures[index];
            c.hunting = false;
            c.last_seen = None;
            c.listening = false;
        }
        if notices {
            if self.player.hidden {
                self.player.hidden = false;
                self.say(Tone::Info, format!("The {} spots you!", stats.name));
            }
            let c = &mut self.creatures[index];
            c.hunting = true;
            c.lost = 0;
            c.last_seen = Some(me);
            c.listening = false;
        } else if listens {
            let was_hunting = self.creatures[index].hunting;
            let source = sound_notices.expect("a sound was heard");
            let c = &mut self.creatures[index];
            c.hunting = true;
            c.lost = 0;
            c.last_seen = Some(source);
            c.listening = true;
            if !was_hunting {
                let text = if self.sees(pos) {
                    format!("The {} tracks your footsteps.", stats.name)
                } else {
                    "Something tracks your footsteps.".to_owned()
                };
                self.say(Tone::Info, text);
            }
        } else if self.creatures[index].hunting {
            let c = &mut self.creatures[index];
            c.lost += 1;
            if c.lost > PATIENCE {
                c.hunting = false;
                c.last_seen = None;
                c.listening = false;
            }
        }
        if stats.regen > 0 {
            let c = &mut self.creatures[index];
            c.hp = (c.hp + stats.regen).min(stats.hp);
        }
        let c = &self.creatures[index];
        if c.hunting && notices && close {
            self.creature_strike(index, stats.hit, stats.damage, None);
        } else if c.hunting
            && notices
            && let Some((range, damage)) = stats.shot
            && distance(pos, me) <= range
        {
            self.creature_strike(index, stats.hit, damage, Some("arrow"));
        } else if c.hunting {
            if c.listening {
                if let Some(goal) = c.last_seen {
                    self.tremor_investigate(index, goal);
                }
            } else {
                let goal = c.last_seen.unwrap_or(me);
                self.creature_step(index, goal);
            }
        } else if distance(pos, home) > 4 {
            self.creature_step(index, home);
        } else if self.rng.chance(30) {
            let dir = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw][self.rng.below(8) as usize];
            let (dx, dy) = dir.delta();
            let next = (pos.0 + dx, pos.1 + dy);
            if distance(next, self.creatures[index].home) <= 4 && self.open_for_creature(index, next) {
                self.creatures[index].pos = next;
            }
        }
    }

    fn creature_strike(&mut self, index: usize, hit: i32, (lo, hi): (i32, i32), missile: Option<&str>) {
        let name = self.creatures[index].stats().name;
        let natural = self.rng.d20();
        let ward = self.player.ward_protection(missile.is_some());
        let roll = natural + hit;
        let ac = self.player.ac();
        if natural != 20 && roll < ac + ward {
            let prevented = ward > 0 && roll >= ac;
            if prevented {
                let text = match missile {
                    Some(m) => format!("Your ward turns aside the {name}'s {m}."),
                    None => format!("Your ward turns aside the {name}'s blow."),
                };
                self.say(Tone::Miss, text);
                let useful = self.teach_utility(index, Practice::Warding, 5);
                let credit = std::mem::take(&mut self.player.shield_focus_credit);
                let threat = self.threat(index, Practice::Focus);
                if useful && credit > 0 && threat >= 10 {
                    self.learn_practice(Practice::Focus, credit.saturating_mul(threat), threat);
                }
            } else {
                let text = match missile {
                    Some(m) => format!("The {name}'s {m} whistles past you."),
                    None => format!("The {name} misses you."),
                };
                self.say(Tone::Miss, text);
            }
            return;
        }
        let damage = self.rng.range(lo, hi);
        let threat = self.threat(index, Practice::Vitality);
        let restoration_threat = self.threat(index, Practice::Restoration);
        let creature_id = self.creatures[index].id;
        let capacity = self.creatures[index].stats().hp;
        let taken = damage.min(self.player.hp.max(0));
        self.player.hp -= damage;
        self.prune_injury_sources();
        self.player.record_injury(creature_id, restoration_threat, capacity, taken);
        if threat >= 10 {
            let useful = self.creatures[index].spend_vitality(taken);
            if useful > 0 {
                self.learn_practice(Practice::Vitality, useful.saturating_mul(threat), threat);
            }
        }
        let text = match missile {
            Some(m) => format!("The {name}'s {m} hits you for {damage}!"),
            None => format!("The {name} hits you for {damage}!"),
        };
        self.say(Tone::Hurt, text);
    }

    fn open_for_creature(&self, index: usize, pos: Pos) -> bool {
        let level = self.level();
        let access = self.creatures[index].phenotype().map_or_else(
            || level.tile(pos).walkable(),
            |p| {
                !level.safe(pos)
                    && !matches!(level.tile(pos), Tile::Rock | Tile::Wall)
                    && p.access.allows(level.ground(pos))
            },
        );
        access
            && level.zone(pos) == Zone::Open
            && pos != self.player.pos
            && !matches!(level.tile(pos), Tile::StairsDown | Tile::StairsUp)
            && !self.creatures.iter().any(|c| c.at == self.player.at && c.pos == pos)
    }

    fn creature_step(&mut self, index: usize, goal: Pos) {
        let (pos, home) = (self.creatures[index].pos, self.creatures[index].home);
        let leash = if self.player.at == LevelId::Surface { WILD_LEASH } else { DEEP_LEASH };
        let score = |p: Pos| (distance(p, goal), (p.0 - goal.0).abs() + (p.1 - goal.1).abs());
        let best = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw]
            .into_iter()
            .map(|d| (pos.0 + d.delta().0, pos.1 + d.delta().1))
            .filter(|&p| self.open_for_creature(index, p) && distance(p, home) <= leash.max(distance(pos, home)))
            .min_by_key(|&p| score(p));
        if let Some(next) = best
            && score(next) < score(pos)
        {
            self.creatures[index].pos = next;
        } else if Some(pos) == self.creatures[index].last_seen || best.is_none() {
            self.creatures[index].last_seen = None;
        }
    }

    fn tremor_investigate(&mut self, index: usize, goal: Pos) {
        if let Some((_, Some(next))) = self.tremor_route(index, goal, TREMOR_LOUD_RANGE) {
            self.creatures[index].pos = next;
        }
    }

    fn die(&mut self) {
        let p = &mut self.player;
        p.deaths += 1;
        let (at, pos, gold) = (p.at, p.pos, std::mem::take(&mut p.gold));
        p.hp = p.max_hp;
        p.focus = p.max_focus;
        p.nocked = false;
        p.chanted = None;
        p.hidden = false;
        p.shield_focus_credit = 0;
        p.injuries.clear();
        p.at = LevelId::Surface;
        p.pos = self.revival;
        if gold > 0 {
            self.drop_at(at, pos, Item::Gold(gold));
        }
        for c in &mut self.creatures {
            c.hunting = false;
            c.last_seen = None;
            c.listening = false;
        }
        self.say(Tone::Death, "You have died!");
        self.say(Tone::System, "You wake on the temple floor. Your gold lies where you fell.");
    }

    fn recover(&mut self) {
        let p = &mut self.player;
        if p.shield > 0 {
            p.shield -= 1;
            if p.shield == 0 {
                p.shield_focus_credit = 0;
                self.say(Tone::Info, "Your ward fades.");
            }
        }
    }

    /// Sets up a floor the first time anyone reaches it: its memory, spawn points, and the treasure lying
    /// about, which grows richer and more plentiful with depth.
    fn open_deep(&mut self, deep: Deep) {
        let at = deep.level();
        if self.deeps.iter().any(|d| d.at == at) {
            return;
        }
        let cells = (Deep::WIDTH * Deep::HEIGHT) as usize;
        self.deeps.push(DeepMemory { at, seen: vec![0; cells.div_ceil(64)] });
        for (index, (_, den)) in deep.spawns().into_iter().enumerate() {
            self.spawns.push(SpawnState { at, index, den, occupant: None, ready: 0 });
        }
        let keys = [deep.town.origin.0 as u64, deep.town.origin.1 as u64, u64::from(deep.depth)];
        let mut places = blah_core::Rng::keyed(self.seed, b"deeploot", &keys);
        let mut dice = Rng::new(places.next_u64());
        let finds = 1 + dice.below(2) + (deep.depth / 3).min(4);
        for _ in 0..finds {
            let value = 20 * deep.depth + dice.below(20 * deep.depth);
            let pos = deep.hiding_place(&mut places);
            for item in treasure(value, 100, &mut dice) {
                self.drop_at(at, pos, item);
            }
        }
    }

    fn respawn(&mut self) {
        let LevelId::Deep(town, depth) = self.player.at else { return };
        let spawns = Deep::new(town, depth).spawns();
        for i in 0..self.spawns.len() {
            let s = &self.spawns[i];
            if s.at != self.player.at || s.occupant.is_some() || self.round < s.ready {
                continue;
            }
            if distance(spawns[s.index].0, self.player.pos) <= SIGHT + 2 {
                continue;
            }
            self.spawn(i, spawns[s.index].0);
        }
    }

    fn spawn(&mut self, i: usize, pos: Pos) {
        let s = &self.spawns[i];
        let LevelId::Deep(town, depth) = s.at else { unreachable!("spawn points are below ground") };
        let mut rng = blah_core::Rng::keyed(
            self.seed,
            b"deepborn",
            &[town.origin.0 as u64, town.origin.1 as u64, u64::from(depth), s.index as u64, self.round],
        );
        // A den keeps the strongest of several individuals; anything that cannot walk stone is not born here.
        let draws = if s.den { 4 } else { 1 };
        let wild = (0..draws)
            .map(|_| self.life.draw_deep(self.seed, town, depth, &mut rng))
            .filter(|w| w.phenotype().access.allows(&crate::wildlife::DEEP))
            .max_by_key(|w| {
                let p = w.phenotype().stats;
                p.hp + p.attack
            });
        let s_at = s.at;
        let Some(wild) = wild else {
            self.spawns[i].ready = self.round + 1;
            return;
        };
        let id = self.next_id;
        self.next_id += 1;
        let hp = wild.stats().hp;
        self.creatures.push(Creature {
            id,
            kind: Kind::Wild,
            wild: Some(wild),
            energy: 0,
            at: s_at,
            pos,
            hp,
            home: pos,
            hunting: false,
            last_seen: None,
            lost: 0,
            listening: false,
            stunned: 0,
            learning_damage_left: hp,
            vitality_lessons_left: hp,
            stealth_lessons_left: 12,
            utility_lessons_left: 12,
            revealed_by_attack: false,
            spawn: Some(i),
        });
        self.spawns[i].occupant = Some(id);
    }

    fn update_seen(&mut self) {
        let level = self.level();
        let me = self.player.pos;
        let memory = self.deeps.iter().position(|d| d.at == self.player.at);
        for y in me.1 - SIGHT..=me.1 + SIGHT {
            for x in me.0 - SIGHT..=me.0 + SIGHT {
                let pos = (x, y);
                if !level.contains(pos) || !level.sees(me, pos, SIGHT) {
                    continue;
                }
                if let (Some(deep), Some(m)) = (level.deep(), memory) {
                    let i = deep.cell_index(pos).expect("contained");
                    self.deeps[m].seen[i / 64] |= 1 << (i % 64);
                } else {
                    let (chunk, word, bit) = memory_cell(pos);
                    self.explored.entry(chunk.0).or_default().entry(chunk.1).or_default()[word] |= bit;
                }
            }
        }
        self.discover_wilderness();
    }

    fn discover_wilderness(&mut self) {
        if self.player.at == LevelId::Surface {
            let level = self.level();
            let me = self.player.pos;
            for sy in (me.1 - SIGHT).div_euclid(SECTOR)..=(me.1 + SIGHT).div_euclid(SECTOR) {
                for sx in (me.0 - SIGHT).div_euclid(SECTOR)..=(me.0 + SIGHT).div_euclid(SECTOR) {
                    if let Some(site) = Landmark::generate(self.seed, (sx, sy), &level)
                        && level.sees(me, site.pos, SIGHT)
                        && !self.landmarks.iter().any(|known| known.pos == site.pos)
                    {
                        self.landmarks.push(site);
                        let (chunk, word, bit) = memory_cell(site.pos);
                        self.explored.entry(chunk.0).or_default().entry(chunk.1).or_default()[word] |= bit;
                        self.say(
                            Tone::Gain,
                            format!("You discover a site: {}. Its cache is untouched.", site.kind.name()),
                        );
                    }
                }
            }
            for sy in (me.1 - SIGHT).div_euclid(HERB_SECTOR)..=(me.1 + SIGHT).div_euclid(HERB_SECTOR) {
                for sx in (me.0 - SIGHT).div_euclid(HERB_SECTOR)..=(me.0 + SIGHT).div_euclid(HERB_SECTOR) {
                    if let Some(patch) = HerbPatch::generate(self.seed, (sx, sy), &level)
                        && level.sees(me, patch.pos, SIGHT)
                        && !self.herbs.iter().any(|known| known.pos == patch.pos)
                    {
                        self.herbs.push(patch);
                        let (chunk, word, bit) = memory_cell(patch.pos);
                        self.explored.entry(chunk.0).or_default().entry(chunk.1).or_default()[word] |= bit;
                        self.say(
                            Tone::Info,
                            format!("You find {} growing nearby; {}.", patch.herb.name(), patch.herb.description()),
                        );
                    }
                }
            }
        }
    }
}
fn memory_cell((x, y): Pos) -> (Pos, usize, u64) {
    let index = (y.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize;
    ((x.div_euclid(16), y.div_euclid(16)), index / 64, 1_u64 << (index % 64))
}

#[cfg(test)]
impl Game {
    pub(crate) fn player_mut(&mut self) -> &mut Player {
        &mut self.player
    }

    /// Places a fresh creature of `kind` at `pos` on the character's map, outside any spawn point.
    pub(crate) fn place(&mut self, kind: Kind, pos: Pos) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let at = self.player.at;
        let hp = kind.stats().hp;
        self.creatures.push(Creature {
            id,
            kind,
            wild: None,
            energy: 0,
            at,
            pos,
            hp,
            home: pos,
            hunting: false,
            last_seen: None,
            lost: 0,
            listening: false,
            stunned: 0,
            learning_damage_left: hp,
            vitality_lessons_left: hp,
            stealth_lessons_left: 12,
            utility_lessons_left: 12,
            revealed_by_attack: false,
            spawn: None,
        });
        id
    }

    pub(crate) fn clear_creatures(&mut self) {
        self.creatures.clear();
        self.wild_ready = u64::MAX;
        self.spawns.clear();
        self.deeps.clear();
    }
}

#[cfg(test)]
mod wildlife_tests {
    use super::*;
    use crate::wildlife::{CaveTraits, Lineage};
    use blah_core::{Genome, World};

    fn made() -> Game {
        let mut game = Game::new(7);
        game.answer("Wren");
        game.clear_creatures();
        game.player.hp = 100_000;
        game.player.max_hp = 100_000;
        game
    }

    fn clearing(game: &Game) -> Pos {
        for y in -100..=100 {
            for x in -100..=100 {
                let pos = (x, y);
                if (-1..=1).all(|dy| {
                    (-1..=1).all(|dx| {
                        let at = (x + dx, y + dy);
                        !game.level().authored(at)
                            && game.level().tile(at).walkable()
                            && !game.terrain.biome(at).biome().water
                    })
                }) {
                    return pos;
                }
            }
        }
        panic!("seed 7 has a clearing outside town");
    }

    fn genome(size: f32, aggression: f32, water: f32) -> Genome {
        Genome::new([size, 1.0, 0.0, 0.5, 0.5, water, aggression, 0.3])
    }

    fn add(game: &mut Game, pos: Pos, genome: Genome) {
        let mut wild = game.life.draw(game.seed, game.terrain.world_pos(pos), &mut blah_core::Rng::new(2));
        wild.genome = genome;
        let hp = wild.stats().hp;
        game.place(Kind::Wild, pos);
        let creature = game.creatures.last_mut().unwrap();
        creature.wild = Some(wild);
        creature.hp = hp;
    }

    fn cave_game() -> (Game, Town, Deep) {
        let mut game = made();
        let town = game.towns.home;
        let deep = Deep::new(town, 1);
        game.player.at = deep.level();
        game.player.pos = deep.stairs();
        game.clear_creatures();
        game.player.pos = (0..Deep::HEIGHT)
            .flat_map(|y| (0..Deep::WIDTH).map(move |x| (x, y)))
            .find(|&pos| deep.tile(pos) == Tile::Floor && !deep.landing(pos))
            .expect("a Deep floor outside the landing");
        (game, town, deep)
    }

    fn horizontal_run(game: &Game, deep: Deep, len: i32) -> Pos {
        for y in 0..Deep::HEIGHT {
            for x in 0..Deep::WIDTH - len {
                if (x..=x + len).all(|step| {
                    let pos = (step, y);
                    game.level().tile(pos) == Tile::Floor && !deep.landing(pos)
                }) {
                    return (x, y);
                }
            }
        }
        panic!("a Deep room contains a horizontal run of {len} floor cells");
    }

    fn floor_path_distance(deep: Deep, from: Pos, to: Pos, limit: usize) -> Option<usize> {
        let mut queue = VecDeque::from([(from, 0_usize)]);
        let mut visited = BTreeSet::from([from]);
        while let Some((pos, steps)) = queue.pop_front() {
            if pos == to {
                return Some(steps);
            }
            if steps == limit {
                continue;
            }
            for dir in [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw] {
                let (dx, dy) = dir.delta();
                let next = (pos.0 + dx, pos.1 + dy);
                if deep.tile(next) == Tile::Floor && !deep.landing(next) && visited.insert(next) {
                    queue.push_back((next, steps + 1));
                }
            }
        }
        None
    }

    fn corner_step(game: &Game, deep: Deep, desired: usize) -> (Pos, Pos, Pos, Dir) {
        for y in 1..Deep::HEIGHT - 1 {
            for x in 1..Deep::WIDTH - 1 {
                let heard = (x, y);
                if deep.tile(heard) != Tile::Floor || deep.landing(heard) {
                    continue;
                }
                let Some(start) =
                    [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw].into_iter().find_map(|dir| {
                        let (dx, dy) = dir.delta();
                        let from = (heard.0 - dx, heard.1 - dy);
                        (from != heard && deep.tile(from) == Tile::Floor && !deep.landing(from)).then_some((from, dir))
                    })
                else {
                    continue;
                };
                for cy in 1..Deep::HEIGHT - 1 {
                    for cx in 1..Deep::WIDTH - 1 {
                        let creature = (cx, cy);
                        let score =
                            |pos: Pos| (distance(pos, heard), (pos.0 - heard.0).abs() + (pos.1 - heard.1).abs());
                        let greedy_progress = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw]
                            .into_iter()
                            .any(|dir| {
                                let (dx, dy) = dir.delta();
                                let next = (creature.0 + dx, creature.1 + dy);
                                deep.tile(next) == Tile::Floor
                                    && !deep.landing(next)
                                    && next != heard
                                    && score(next) < score(creature)
                            });
                        if deep.tile(creature) != Tile::Floor
                            || deep.landing(creature)
                            || creature == start.0
                            || game.level().sees(creature, heard, SIGHT)
                            || game.level().sees(creature, start.0, SIGHT)
                            || (desired >= 4 && greedy_progress)
                            || floor_path_distance(deep, creature, heard, desired).is_none_or(|d| d != desired)
                        {
                            continue;
                        }
                        return (creature, start.0, heard, start.1);
                    }
                }
            }
        }
        panic!("a Deep corner has a floor route of {desired} cells but no line of sight");
    }

    fn deep_genome(limbs: f32, covering: f32, cold: f32) -> Genome {
        Genome::new([0.5, limbs, covering, 0.5, cold, 0.1, 0.9, 0.2])
    }

    fn add_deep(game: &mut Game, pos: Pos, genome: Genome) -> usize {
        let LevelId::Deep(town, depth) = game.player.at else { panic!("Deep fixture") };
        let mut wild = game.life.draw_deep(game.seed, town, depth, &mut blah_core::Rng::new(19));
        wild.genome = genome;
        let hp = wild.stats().hp;
        let id = game.place(Kind::Wild, pos);
        let index = game.creatures.iter().position(|creature| creature.id == id).unwrap();
        let creature = &mut game.creatures[index];
        creature.wild = Some(wild);
        creature.hp = hp;
        creature.learning_damage_left = hp;
        creature.vitality_lessons_left = hp;
        index
    }

    #[test]
    fn cave_traits_are_deep_only_expressions_of_body_genes() {
        let (mut game, town, _) = cave_game();
        let clade = game.life.deep_species(game.seed, town, 1)[0].clade;
        let deep =
            |genome| Wildlife { lineage: Lineage::Deep { town, depth: 1, clade }, name: "fixture".into(), genome };
        assert!(deep(deep_genome(0.6, 0.5, 0.0)).cave_traits().tremorsense, "six-legged chitin");
        assert!(deep(deep_genome(0.6, 0.7, 0.0)).cave_traits().tremorsense, "six-legged scales");
        assert!(!deep(deep_genome(0.59, 0.5, 0.0)).cave_traits().tremorsense, "four legs are not enough");
        assert!(!deep(deep_genome(0.6, 0.1, 0.0)).cave_traits().tremorsense, "fur does not tremor-sense");

        let glowing = deep(deep_genome(0.0, 0.9, 0.5));
        assert_eq!(glowing.cave_traits(), CaveTraits { tremorsense: false, luminous: true });
        assert!(!deep(deep_genome(0.0, 0.9, 0.49)).cave_traits().luminous, "cold threshold is inclusive at 0.5");
        assert!(!deep(deep_genome(0.0, 0.7, 0.9)).cave_traits().luminous, "only slime glows");
        assert_eq!(CaveTraits::default(), CaveTraits { tremorsense: false, luminous: false });

        let mut surface = game.life.draw(game.seed, (0, 0), &mut blah_core::Rng::new(20));
        surface.genome = deep_genome(0.6, 0.9, 0.9);
        assert_eq!(surface.cave_traits(), CaveTraits::default());
    }

    #[test]
    fn evolved_deep_pools_express_both_cave_traits_across_floors_and_seeds() {
        let (mut saw_tremorsense, mut saw_luminous, mut saw_neither) = (false, false, false);
        for seed in [3, 7, 42, 91] {
            let mut game = Game::new(seed);
            let town = game.towns.home;
            for depth in [1, 4, 8] {
                let species = game.life.deep_species(seed, town, depth);
                for species in species {
                    for &genome in &species.pool {
                        let wild = Wildlife {
                            lineage: Lineage::Deep { town, depth, clade: species.clade },
                            name: species.name.clone(),
                            genome,
                        };
                        let traits = wild.cave_traits();
                        saw_tremorsense |= traits.tremorsense;
                        saw_luminous |= traits.luminous;
                        saw_neither |= !traits.tremorsense && !traits.luminous;
                        assert!(!(traits.tremorsense && traits.luminous), "each trait is not universal");
                    }
                }
            }
        }
        assert!(saw_tremorsense && saw_luminous && saw_neither);
    }

    #[test]
    fn cave_light_is_local_occluded_living_and_confined_to_its_floor() {
        let (mut game, town, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 4);
        let source = (x, y);
        let near = (x + 2, y);
        let far = (x + 3, y);
        let index = add_deep(&mut game, source, deep_genome(0.1, 0.9, 0.5));
        assert!(game.creatures[index].cave_traits().luminous);
        assert!(game.illuminated(near));
        assert!(game.illuminated(near), "lighting queries are stable and side-effect free");
        assert!(!game.illuminated(far), "light ends at radius two");

        let blocked = (1..Deep::HEIGHT - 1)
            .flat_map(|by| (1..Deep::WIDTH - 1).map(move |bx| (bx, by)))
            .find_map(|wall| {
                (1..Deep::HEIGHT - 1)
                    .flat_map(|ty| (1..Deep::WIDTH - 1).map(move |tx| (tx, ty)))
                    .find(|&target| {
                        distance(wall, target) == 2
                            && deep.tile(wall).walkable()
                            && deep.tile(target).walkable()
                            && !deep.landing(wall)
                            && !deep.landing(target)
                            && !game.level().sees(wall, target, CAVE_LIGHT_RADIUS)
                    })
                    .map(|target| (wall, target))
            })
            .expect("a Deep wall separates two nearby floor cells");
        game.creatures[index].pos = blocked.0;
        assert!(!game.illuminated(blocked.1), "walls block light around corners");
        let wall = (0..Deep::HEIGHT)
            .flat_map(|wy| (0..Deep::WIDTH).map(move |wx| (wx, wy)))
            .find(|&pos| !game.level().tile(pos).walkable())
            .unwrap();
        assert!(!game.illuminated(wall), "only walkable cells are illuminated");

        game.creatures[index].pos = source;
        game.creatures[index].hp = 0;
        assert!(!game.illuminated(near), "dead life gives no light");
        game.creatures[index].hp = 1;
        game.player.at = LevelId::Deep(town, 2);
        assert!(!game.illuminated(near), "light does not cross floors");
        game.player.at = LevelId::Surface;
        assert!(!game.illuminated(near), "cave light does not reach the surface");
    }

    #[test]
    fn cave_light_blocks_hiding_breaks_concealment_and_can_be_left() {
        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 4);
        let source = (x, y);
        let index = add_deep(&mut game, source, deep_genome(0.1, 0.9, 0.5));
        game.creatures[index].stunned = 100;
        game.player.pos = (x + 3, y);
        game.player.dex = 1_000;
        game.player.hidden = true;

        game.round(Some(Command::Move(vec![Dir::W])));
        assert_eq!(game.player.pos, (x + 2, y));
        assert!(!game.player.hidden, "entering a lit cell reveals the player");
        assert!(!game.footsteps[0].quiet, "a light-revealed step is noisy");
        assert!(game.log.iter().any(|line| line.text.contains("glow drives you out")));

        game.round(Some(Command::Hide));
        assert!(!game.player.hidden, "hiding cannot begin in the light");
        assert!(game.log.iter().any(|line| line.text.contains("glow drives away the shadows")));

        game.round(Some(Command::Move(vec![Dir::E, Dir::E])));
        assert_eq!(game.player.pos, (x + 4, y));
        assert!(!game.illuminated(game.player.pos), "moving out leaves the light radius");
        game.round(Some(Command::Hide));
        assert!(game.player.hidden, "hiding works again after leaving the light");
    }

    #[test]
    fn a_luminous_creature_approaching_reveals_a_hidden_player_after_its_move() {
        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 4);
        game.player.pos = (x, y);
        game.player.hidden = true;
        let index = add_deep(&mut game, (x + 3, y), deep_genome(0.1, 0.9, 0.5));
        game.creatures[index].energy = 9;
        game.creatures[index].hunting = true;
        game.creatures[index].last_seen = Some(game.player.pos);
        assert!(!game.illuminated(game.player.pos));

        game.round(Some(Command::Rest));
        assert_eq!(game.creatures[index].pos, (x + 2, y));
        assert!(game.illuminated(game.player.pos));
        assert!(!game.player.hidden, "newly arrived light reveals before the next creature action");
        assert!(game.encounter_active(index), "revealing light counts as an active encounter");
    }

    #[test]
    fn tremor_investigation_follows_a_route_a_greedy_step_cannot_take() {
        let (mut game, _, deep) = cave_game();
        let (creature, start, step, direction) = corner_step(&game, deep, 4);
        let index = add_deep(&mut game, creature, deep_genome(0.6, 0.5, 0.0));
        game.player.pos = start;
        let before = floor_path_distance(deep, creature, step, 5).unwrap();
        let hp = game.player.hp;

        game.round(Some(Command::Move(vec![direction])));
        assert!(game.creatures[index].hunting && game.creatures[index].listening);
        assert_eq!(game.creatures[index].last_seen, Some(step));
        assert_ne!(game.creatures[index].pos, creature, "BFS moves even where greedy pursuit makes no progress");
        assert_eq!(floor_path_distance(deep, game.creatures[index].pos, step, 5), Some(before - 1));
        assert_eq!(game.player.hp, hp, "the route does not authorize an attack through a wall");
    }

    #[test]
    fn luminous_life_cannot_enter_or_attack_a_deep_landing() {
        let (mut game, _, deep) = cave_game();
        let pair = (0..Deep::HEIGHT)
            .flat_map(|y| (0..Deep::WIDTH).map(move |x| (x, y)))
            .find_map(|safe| {
                if !deep.landing(safe) {
                    return None;
                }
                (0..Deep::HEIGHT)
                    .flat_map(|y| (0..Deep::WIDTH).map(move |x| (x, y)))
                    .find(|&source| {
                        !deep.landing(source)
                            && deep.tile(source) == Tile::Floor
                            && distance(source, safe) <= CAVE_LIGHT_RADIUS
                            && game.level().sees(source, safe, CAVE_LIGHT_RADIUS)
                    })
                    .map(|source| (source, safe))
            })
            .expect("a corridor can light the edge of a safe landing");
        let index = add_deep(&mut game, pair.0, deep_genome(0.1, 0.9, 0.5));
        game.player.pos = pair.1;
        game.player.hidden = true;
        game.creatures[index].hunting = true;
        game.creatures[index].last_seen = Some(pair.1);
        let hp = game.player.hp;
        assert!(deep.landing(game.player.pos));
        assert!(game.illuminated(game.player.pos));
        assert!(!game.open_for_creature(index, game.player.pos));

        game.round(Some(Command::Rest));
        assert_eq!(game.player.hp, hp, "the light does not break landing protection");
        assert!(!deep.landing(game.creatures[index].pos));
        assert!(!game.open_for_creature(index, game.player.pos));
    }

    #[test]
    fn tremorsense_hears_real_steps_around_corners_with_quiet_and_loud_ranges() {
        for (hidden, heard) in [(true, false), (false, true)] {
            let (mut game, _, deep) = cave_game();
            let (creature, start, step, direction) = corner_step(&game, deep, 3);
            let index = add_deep(&mut game, creature, deep_genome(0.6, 0.5, 0.0));
            game.player.pos = start;
            game.player.hidden = hidden;
            game.player.dex = 1_000;
            let hp = game.player.hp;
            assert!(!game.level().sees(creature, step, SIGHT), "the source is around a wall");

            game.round(Some(Command::Move(vec![direction])));
            assert_eq!(game.player.pos, step);
            assert_eq!(game.footsteps.len(), 1);
            assert_eq!(game.footsteps[0].quiet, hidden);
            assert_eq!(game.creatures[index].hunting, heard);
            assert_eq!(game.player.hidden, hidden, "hearing never globally reveals the player");
            assert_eq!(game.player.hp, hp, "sound does not authorize an attack through a wall");
            if heard {
                assert!(game.creatures[index].listening);
                assert_eq!(game.creatures[index].last_seen, Some(step));
                assert!(game.creatures[index].pos != creature, "a listener routes toward the footstep");
                assert!(game.log.iter().any(|line| line.text == "Something tracks your footsteps."));
            }
        }
    }

    #[test]
    fn stationary_hidden_players_make_no_tremor_target_and_sound_is_not_an_opening() {
        let (mut still, _, deep) = cave_game();
        let (creature, _, step, _) = corner_step(&still, deep, 3);
        let index = add_deep(&mut still, creature, deep_genome(0.6, 0.5, 0.0));
        still.player.pos = step;
        still.player.hidden = true;
        still.round(Some(Command::Rest));
        assert!(still.footsteps.is_empty());
        assert!(!still.creatures[index].hunting, "a stationary hidden player is not sensed telepathically");
        assert!(still.player.hidden);

        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 2);
        let mut dangerous = deep_genome(0.6, 0.5, 0.0);
        dangerous.set(blah_core::Gene::Size, 1.0);
        let index = add_deep(&mut game, (x, y), dangerous);
        game.creatures[index].energy = 1;
        game.player.pos = (x + 1, y);
        game.player.hidden = true;
        game.player.str = 3;
        game.player.dex = 3;
        game.player.con = 3;
        game.rng = Rng::new(1); // The hidden-step check rolls 46 against its 50% chance.
        let threat = game.threat(index, Practice::Stealth);
        assert!(threat >= 10, "the listener is a meaningful stealth encounter");
        game.round(Some(Command::Move(vec![Dir::E])));
        assert!(game.creatures[index].hunting && game.creatures[index].listening);
        assert!(game.player.hidden, "listening does not clear hiding");
        assert_eq!(game.creatures[index].stealth_lessons_left, 12);
        assert_eq!(game.player.practice.get(Practice::Stealth), blah_core::Skill::default());

        game.creatures[index].hp = 10_000;
        game.creatures[index].learning_damage_left = 10_000;
        let stealth_before = game.player.practice.get(Practice::Stealth);
        game.round(Some(Command::Attack(Some(Target::Label('A')))));
        assert!(!game.player.hidden);
        assert_eq!(game.player.practice.get(Practice::Stealth), stealth_before);
        assert_eq!(game.creatures[index].stealth_lessons_left, 12, "a listening hunter denies the opening lesson");
    }

    #[test]
    fn only_accepted_walk_steps_are_recorded_and_stairs_clear_old_floor_noise() {
        let (mut game, _, deep) = cave_game();
        let (creature, start, _, _) = corner_step(&game, deep, 3);
        let index = add_deep(&mut game, creature, deep_genome(0.6, 0.5, 0.0));
        game.player.pos = start;
        game.player.hidden = true;
        let wall_step = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw]
            .into_iter()
            .find(|dir| {
                let (dx, dy) = dir.delta();
                !game.level().tile((start.0 + dx, start.1 + dy)).walkable()
            })
            .expect("the Deep room has a wall");
        game.round(Some(Command::Move(vec![wall_step])));
        assert!(game.footsteps.is_empty(), "failed movement makes no noise");
        assert!(!game.creatures[index].hunting, "failed movement is not heard");

        game.round(Some(Command::Rest));
        assert!(game.footsteps.is_empty(), "resting does not make a step");
        game.round(Some(Command::Wait));
        assert!(game.footsteps.is_empty(), "waiting does not make a step");
        game.round(Some(Command::Hide));
        assert!(game.footsteps.is_empty(), "changing hiding state does not make a step");

        game.player.at = LevelId::Surface;
        game.player.pos = game.towns.home.stairs();
        game.footsteps.push(Footstep { at: LevelId::Surface, pos: game.player.pos, quiet: false });
        game.round(Some(Command::Down));
        assert!(matches!(game.player.at, LevelId::Deep(..)));
        assert!(game.footsteps.is_empty(), "stairs do not carry footsteps onto another floor");
    }

    #[test]
    fn a_direct_move_cannot_record_or_take_more_than_three_steps() {
        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 5);
        game.player.pos = (x, y);
        game.round(Some(Command::Move(vec![Dir::E; MAX_STEPS + 1])));
        assert_eq!(game.player.pos, (x + MAX_STEPS as i32, y));
        assert_eq!(game.footsteps.len(), MAX_STEPS);
    }

    #[test]
    fn a_listener_follows_the_latest_audible_step_not_the_closest_older_one() {
        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 4);
        let index = add_deep(&mut game, (x, y), deep_genome(0.6, 0.5, 0.0));
        game.player.pos = (x + 3, y);
        game.player.hidden = true;
        game.player.dex = 1_000;
        game.round(Some(Command::Move(vec![Dir::W, Dir::W, Dir::E])));
        assert!(game.creatures[index].listening);
        assert_eq!(game.creatures[index].last_seen, Some((x + 2, y)));
    }

    #[test]
    fn footstep_ranges_include_the_boundary_and_exclude_the_next_cell() {
        for (quiet, range) in [(true, TREMOR_QUIET_RANGE), (false, TREMOR_LOUD_RANGE)] {
            for distance in [range, range + 1] {
                let (mut game, _, deep) = cave_game();
                let (x, y) = horizontal_run(&game, deep, 6);
                let index = add_deep(&mut game, (x, y), deep_genome(0.6, 0.5, 0.0));
                game.creatures[index].stunned = 1;
                game.player.pos = (x + distance as i32 - 1, y);
                game.player.hidden = quiet;
                game.player.dex = 1_000;
                game.round(Some(Command::Move(vec![Dir::E])));
                assert_eq!(game.can_hear_footstep(index, &game.footsteps[0]), (distance == range).then_some(distance));
            }
        }
    }

    #[test]
    fn a_three_step_move_tracks_an_earlier_quiet_source_and_replays_after_save() {
        let (mut game, _, deep) = cave_game();
        let (x, y) = horizontal_run(&game, deep, 4);
        let index = add_deep(&mut game, (x, y), deep_genome(0.6, 0.5, 0.0));
        game.player.pos = (x + 1, y);
        game.player.hidden = true;
        game.player.dex = 1_000;
        game.round(Some(Command::Move(vec![Dir::E, Dir::E, Dir::E])));
        assert_eq!(game.player.pos, (x + 4, y));
        assert_eq!(game.footsteps.len(), 3);
        assert!(game.footsteps.iter().all(|step| step.quiet));
        assert!(game.creatures[index].hunting);
        assert!(game.creatures[index].listening);
        assert_eq!(game.creatures[index].last_seen, Some((x + 2, y)));
        assert_ne!(game.creatures[index].last_seen, Some(game.player.pos), "the unheard final position is not guessed");
        assert_eq!(game.player.hp, 100_000, "following a sound does not strike from afar");
        assert!(game.creatures[index].energy > 0, "the listener retains partial speed energy");

        game.creatures[index].stunned = 2;
        let mut loaded: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        for command in [Some(Command::Rest), Some(Command::Wait), Some(Command::Rest)] {
            game.round(command.clone());
            loaded.round(command);
            assert_eq!(loaded, game, "footstep pursuit and partial speed replay exactly");
        }
    }

    #[test]
    fn surface_population_is_evolved_bounded_offscreen_and_outside_town() {
        let mut game = made();
        game.player.pos = clearing(&game);
        game.wild_ready = 0;
        for _ in 0..30 {
            game.round(Some(Command::Rest));
            if !game.creatures.is_empty() {
                break;
            }
        }
        assert!(!game.creatures.is_empty(), "normal rounds populate the wilderness");
        assert!(game.sighted().is_empty(), "new individuals arrive beyond sight");
        for _ in 0..100 {
            game.round(Some(Command::Rest));
        }
        assert!(!game.creatures.is_empty() && game.creatures.len() <= 12);
        for c in &game.creatures {
            assert_eq!(c.kind, Kind::Wild);
            assert!(!game.level().authored(c.pos));
            let wild = c.wild.as_ref().unwrap();
            let home = game.terrain.world_pos(c.home);
            let crate::Lineage::Region(species) = wild.lineage else { panic!("surface life has a region") };
            assert_eq!(species.region, World::new(7).region_at(home.0, home.1));
            assert!(wild.phenotype().access.allows(game.terrain.biome(c.pos).biome()));
            assert_eq!(c.stats().hp, wild.phenotype().stats.hp);
            assert!(distance(c.pos, c.home) <= WILD_LEASH);
        }
        game.player.pos = (1_000_000, -1_000_000);
        game.round(Some(Command::Rest));
        assert!(game.creatures.iter().all(|c| distance(c.pos, game.player.pos) <= 32));
    }

    #[test]
    fn cold_reload_replays_wildlife_spawns_and_movement_exactly() {
        let mut game = made();
        game.player.pos = clearing(&game);
        game.wild_ready = 0;
        for _ in 0..20 {
            game.round(Some(Command::Rest));
        }
        let mut loaded: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert!(loaded.life.inner_is_empty());
        for _ in 0..80 {
            game.round(Some(Command::Rest));
            loaded.round(Some(Command::Rest));
            assert_eq!(loaded, game);
        }
    }

    #[test]
    fn genome_speed_and_wounded_combat_replay_after_partial_rounds_and_stun() {
        for (size, expected) in [(1.0, 9), (0.0, 13)] {
            let mut game = made();
            let me = clearing(&game);
            game.player.pos = me;
            add(&mut game, (me.0 + 1, me.1), genome(size, 1.0, 0.1));
            game.creatures[0].stunned = 2;
            game.round(Some(Command::Rest));
            game.round(Some(Command::Rest));
            assert_eq!(game.creatures[0].energy, 0);
            game.creatures[0].hp -= 1;
            let mark = game.log_total;
            game.round(Some(Command::Rest));
            let mut loaded: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
            assert!(loaded.creatures[0].energy > 0, "resume with a partly banked action");
            assert_eq!(loaded.creatures[0].hp, loaded.creatures[0].stats().hp - 1);
            for _ in 1..10 {
                game.round(Some(Command::Rest));
                loaded.round(Some(Command::Rest));
                assert_eq!(loaded, game);
            }
            let attacks = game
                .log
                .iter()
                .rev()
                .take((game.log_total - mark) as usize)
                .filter(|l| matches!(l.tone, Tone::Hurt | Tone::Miss))
                .count();
            assert_eq!(attacks, expected);
        }
    }

    #[test]
    fn skittish_flee_territorial_guard_home_and_aggressive_hunt() {
        for (aggression, hunts) in [(0.1, false), (0.5, true), (1.0, true)] {
            let mut game = made();
            let me = clearing(&game);
            game.player.pos = me;
            add(&mut game, (me.0 + 1, me.1), genome(0.5, aggression, 0.1));
            game.creature_turn(0);
            assert_eq!(game.creatures[0].hunting, hunts);
            if !hunts {
                assert!(distance(game.creatures[0].pos, me) > 1);
                assert_eq!(game.player.hp, 100_000);
            }
        }
        let mut game = made();
        let me = clearing(&game);
        game.player.pos = me;
        add(&mut game, (me.0 + 1, me.1), genome(0.5, 0.5, 0.1));
        game.creatures[0].home = (me.0 + 4, me.1);
        game.creature_turn(0);
        assert!(!game.creatures[0].hunting, "territorial life does not attack outside its home radius");
        game.provoke(0);
        game.creature_turn(0);
        assert!(game.creatures[0].hunting, "provoked territorial life fights back");
        game.creatures[0].wild.as_mut().unwrap().genome = genome(0.5, 0.1, 0.1);
        game.player.pos = (me.0 + 30, me.1);
        game.creature_turn(0);
        assert!(!game.creatures[0].hunting, "skittish life does not hunt after losing its attacker");
    }

    #[test]
    fn wildlife_cannot_enter_or_attack_into_town_and_obeys_water_access() {
        let mut game = made();
        game.player.pos = (0, 9);
        add(&mut game, (-1, 9), genome(0.5, 1.0, 0.1));
        game.provoke(0);
        for _ in 0..20 {
            game.round(Some(Command::Rest));
        }
        assert_eq!(game.player.hp, 100_000);
        assert!(!game.level().authored(game.creatures[0].pos));
        assert!(!game.open_for_creature(0, (0, 9)));
        for water in [0.1, 0.6, 1.0] {
            game.creatures[0].wild.as_mut().unwrap().genome = genome(0.5, 1.0, water);
            let p = game.creatures[0].phenotype().unwrap();
            for x in -100..0 {
                let at = (x, -100);
                let expected = !matches!(game.level().tile(at), Tile::Rock | Tile::Wall)
                    && p.access.allows(game.terrain.biome(at).biome())
                    && game.creatures[0].pos != at;
                assert_eq!(game.open_for_creature(0, at), expected);
            }
        }
    }

    #[test]
    fn regional_names_target_and_kills_use_individual_stats_and_loot() {
        let mut game = made();
        let me = clearing(&game);
        game.player.pos = me;
        add(&mut game, (me.0 + 1, me.1), genome(0.5, 1.0, 0.1));
        let stats = game.creatures[0].stats();
        let target = Target::Named { nth: 1, name: stats.name.to_lowercase() };
        assert_eq!(game.find_target(Some(target)), Some(0));
        game.creatures[0].hp -= 1;
        let saved: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(saved.creatures[0], game.creatures[0]);
        game.wound(0, 999, Practice::weapon(game.player.weapon), 0);
        assert_eq!(game.player.xp, stats.xp);
        assert!(game.creatures.is_empty());
        assert!(game.log.iter().any(|l| l.text.contains(&stats.name)));
        assert!(game.piles.iter().flat_map(|p| &p.items).all(|item| matches!(item, Item::Gold(_))));
    }

    fn descend(game: &mut Game, town: Town) {
        game.player.at = LevelId::Surface;
        game.player.pos = town.stairs();
        game.round(Some(Command::Down));
        assert_eq!(game.player.at, LevelId::Deep(town, 1));
        game.round(Some(Command::Rest));
    }

    fn other_town(game: &mut Game) -> Town {
        game.level().towns().find(|t| *t != game.towns.home).expect("seed 7 has a neighboring town")
    }

    #[test]
    fn each_town_has_its_own_deep_whose_life_items_and_memory_persist() {
        let mut game = made();
        let (home, other) = (game.towns.home, other_town(&mut game));
        descend(&mut game, home);
        assert_eq!(game.player.pos, Deep::new(home, 1).stairs());
        assert!(game.log.iter().any(|l| l.text.contains(&format!("the Deep beneath {}, depth 1", home.name()))));
        let below: Vec<Creature> = game.creatures.clone();
        assert!(!below.is_empty(), "the Deep is inhabited");
        for c in &below {
            assert_eq!((c.kind, c.at), (Kind::Wild, LevelId::Deep(home, 1)));
            assert!(matches!(c.wild.as_ref().unwrap().lineage, crate::Lineage::Deep { town, .. } if town == home));
            assert!(!Deep::new(home, 1).landing(c.pos));
        }
        let stairs = game.player.pos;
        game.drop_at(game.player.at, stairs, Item::Gold(5));
        assert!(game.remembers(stairs));
        game.round(Some(Command::Up));
        assert_eq!((game.player.at, game.player.pos), (LevelId::Surface, home.stairs()));

        game.player.pos = other.start();
        game.wild_ready = 0;
        game.round(Some(Command::Rest));
        descend(&mut game, other);
        assert_ne!(Deep::new(other, 1), Deep::new(home, 1));
        assert!(game.creatures.iter().any(|c| c.at == LevelId::Deep(other, 1)));
        assert!(game.pile_at(game.player.pos).is_none(), "another town's Deep has its own floor");
        let mut game: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        let kept: Vec<Creature> = game.creatures.iter().filter(|c| c.at == LevelId::Deep(home, 1)).cloned().collect();
        assert_eq!(kept, below, "nothing below an unvisited town moves or vanishes");

        game.round(Some(Command::Up));
        descend(&mut game, home);
        assert!(game.remembers(stairs) && game.pile_at(stairs).is_some());
        for c in &below {
            assert!(game.creatures.iter().any(|now| now.id == c.id), "the same individuals are still down here");
        }
    }

    #[test]
    fn deep_life_keeps_off_the_landing_and_cannot_strike_into_it() {
        let mut game = made();
        let home = game.towns.home;
        descend(&mut game, home);
        let deep = Deep::new(home, 1);
        let mut edge = None;
        for y in 0..Deep::HEIGHT {
            for x in 0..Deep::WIDTH {
                if deep.landing((x, y)) {
                    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
                        let out = (x + dx, y + dy);
                        if !deep.landing(out) && deep.tile(out).walkable() {
                            edge = Some(((x, y), out));
                        }
                    }
                }
            }
        }
        let (inside, outside) = edge.expect("corridors leave the landing");
        game.creatures.truncate(1);
        game.creatures[0].pos = outside;
        game.creatures[0].home = outside;
        game.creatures[0].wild.as_mut().unwrap().genome = genome(0.5, 1.0, 0.1);
        game.player.pos = inside;
        game.provoke(0);
        for _ in 0..30 {
            game.round(Some(Command::Rest));
            assert!(!deep.landing(game.creatures[0].pos));
        }
        assert_eq!(game.player.hp, 100_000);
        // Off the landing, the same creature crosses the Deep's floor to fight.
        game.player.pos = outside;
        let level = game.level();
        let start = (0..Deep::HEIGHT)
            .flat_map(|y| (0..Deep::WIDTH).map(move |x| (x, y)))
            .find(|&p| {
                (3..=5).contains(&distance(p, outside))
                    && !deep.landing(p)
                    && level.tile(p) == Tile::Floor
                    && level.sees(p, outside, 5)
                    && crate::map::line(p, outside).iter().all(|&c| !deep.landing(c) && level.tile(c).walkable())
            })
            .expect("some floor off the landing has a clear walk to its edge");
        game.creatures[0].pos = start;
        game.creatures[0].home = start;
        for _ in 0..15 {
            game.round(Some(Command::Wait));
        }
        assert!(game.log.iter().any(|l| matches!(l.tone, Tone::Hurt | Tone::Miss)), "off the landing it fights");
    }

    #[test]
    fn the_den_keeper_is_the_strongest_draw_and_guards_a_hoard() {
        let mut game = made();
        let home = game.towns.home;
        descend(&mut game, home);
        let den = game.spawns.iter().position(|s| s.den).expect("every Deep has a den");
        let deep = Deep::new(game.towns.home, 1);
        if game.spawns[den].occupant.is_none() {
            game.player.pos = deep.stairs();
            game.spawn(den, deep.spawns()[game.spawns[den].index].0);
        }
        let id = game.spawns[den].occupant.unwrap();
        let index = game.creatures.iter().position(|c| c.id == id).unwrap();
        let at = game.creatures[index].pos;
        game.wound(index, 999, Practice::weapon(game.player.weapon), 0);
        let pile = game.piles.iter().find(|p| p.pos == at && p.at == game.player.at).unwrap();
        assert!(pile.items.contains(&Item::Potion));
        assert!(pile.items.iter().any(|i| !matches!(i, Item::Potion | Item::Gold(_))), "a hoard holds gear");
        assert_eq!(game.spawns[den].ready, game.round + DEN_RESPAWN);
    }

    #[test]
    fn stairs_lead_floor_by_floor_down_and_back_to_the_stairs_taken() {
        let mut game = made();
        let home = game.towns.home;
        descend(&mut game, home);
        for depth in 1..=3 {
            let here = Deep::new(home, depth);
            game.player.pos = here.stairs_down();
            assert!(game.level().safe(game.player.pos), "the lower landing is safe too");
            game.round(Some(Command::Down));
            assert_eq!(game.player.at, LevelId::Deep(home, depth + 1));
            assert_eq!(game.player.pos, Deep::new(home, depth + 1).stairs());
            game.round(Some(Command::Rest));
            let born: Vec<u32> = game
                .creatures
                .iter()
                .filter(|c| c.at == game.player.at)
                .filter_map(|c| c.wild.as_ref())
                .map(Wildlife::depth)
                .collect();
            assert!(!born.is_empty() && born.iter().all(|&d| d == depth + 1));
        }
        let mut game: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        game.round(Some(Command::Up));
        assert_eq!((game.player.at, game.player.pos), (LevelId::Deep(home, 3), Deep::new(home, 3).stairs_down()));
        assert!(game.log.iter().any(|l| l.text == "You climb back up to depth 3."));
        game.player.pos = Deep::new(home, 3).stairs();
        game.round(Some(Command::Up));
        assert_eq!(game.player.at, LevelId::Deep(home, 2));
    }

    #[test]
    fn floors_hold_treasure_that_grows_with_depth_and_stays_until_taken() {
        let mut game = made();
        let home = game.towns.home;
        let value = |game: &Game, depth: u32| -> u32 {
            game.piles
                .iter()
                .filter(|p| p.at == LevelId::Deep(home, depth))
                .flat_map(|p| &p.items)
                .map(|&i| match i {
                    Item::Gold(n) => n,
                    other => crate::data::SHOP.iter().find(|s| s.0 == other).map_or(300, |s| s.1),
                })
                .sum()
        };
        for depth in [1, 2, 9, 10] {
            game.open_deep(Deep::new(home, depth));
            let deep = Deep::new(home, depth);
            for pile in game.piles.iter().filter(|p| p.at == deep.level()) {
                assert!(deep.tile(pile.pos) == Tile::Floor && !deep.landing(pile.pos));
            }
        }
        assert!(value(&game, 1) > 0);
        assert!(value(&game, 9) + value(&game, 10) > 5 * (value(&game, 1) + value(&game, 2)));
        let before = game.piles.clone();
        game.open_deep(Deep::new(home, 1));
        assert_eq!(game.piles, before, "a floor is stocked once");
    }
}

#[cfg(test)]
mod progression_tests {
    use super::*;
    use crate::practice::ceiling;
    use blah_core::{Rank, Skill};

    fn made(seed: u64) -> Game {
        let mut game = Game::new(seed);
        game.answer("Wren");
        game.clear_creatures();
        game
    }

    #[test]
    fn relic_focus_cost_survives_earned_capacity_and_death_without_swap_refills() {
        let mut game = made(7);
        game.player.sack.push(Item::Relic(Relic::Stillstone));
        game.round(Some(Command::Wear("stillstone".into())));
        game.player.focus = 5;
        game.learn_practice(Practice::Focus, 1_000, 200);
        assert_eq!(game.player.practice.get(Practice::Focus).rank(), Rank::Practiced);
        assert_eq!((game.player.focus, game.player.max_focus), (5, 22));
        game.round(Some(Command::Remove("relic".into())));
        assert_eq!((game.player.focus, game.player.max_focus), (5, 26));
        game.round(Some(Command::Wear("stillstone".into())));
        game.learn_practice(Practice::Focus, 29_000, 200);
        assert_eq!(game.player.practice.get(Practice::Focus).rank(), Rank::Expert);
        assert_eq!((game.player.focus, game.player.max_focus), (5, 28));
        game.player.hp = 0;
        game.die();
        assert_eq!(game.player.relic, Some(Relic::Stillstone));
        assert_eq!((game.player.focus, game.player.max_focus), (28, 28));
        game.round(Some(Command::Remove("relic".into())));
        assert_eq!((game.player.focus, game.player.max_focus), (28, 32));
    }

    #[test]
    fn relic_changes_take_exposed_rounds_and_active_wards_use_current_equipment() {
        let mut game = made(7);
        game.player.dex = 10;
        game.player.armor = None;
        game.player.shield = 10;
        let pos = game.player.pos;
        let id = game.place(Kind::Orc, (pos.0 + 1, pos.1));
        let index = game.creatures.iter().position(|c| c.id == id).unwrap();
        game.creatures[index].hunting = true;
        game.creatures[index].last_seen = Some(pos);
        game.player.sack.push(Item::Relic(Relic::Stillstone));
        // +4 Orc accuracy: natural 10 beats the ordinary ward (AC 13), not Stillstone (AC 15).
        game.rng = rng_for_d20(10);
        let health = game.player.hp;
        game.round(Some(Command::Wear("stillstone".into())));
        assert_eq!(game.player.hp, health);
        assert!(game.log.iter().any(|line| line.text.contains("ward turns aside")));
        game.rng = rng_for_d20(10);
        game.round(Some(Command::Remove("relic".into())));
        assert!(game.player.hp < health, "removal spends a round and ends protection immediately");
        assert!(game.player.shield > 0, "an old Shield is not a snapshot of the relic bonus");
    }

    #[test]
    fn stillstone_recovery_respects_reduced_capacity_and_removed_relics_can_be_sold() {
        let mut game = made(7);
        game.player.sack.push(Item::Relic(Relic::Stillstone));
        game.round(Some(Command::Wear("stillstone".into())));
        game.player.focus = 19;
        game.round(Some(Command::Rest));
        assert_eq!((game.player.focus, game.player.max_focus), (20, 20));
        game.player.focus = 19;
        game.player.sack.push(Item::Herb(crate::Herb::Bogmint));
        game.round(Some(Command::Use("bogmint".into())));
        assert_eq!((game.player.focus, game.player.max_focus), (20, 20));
        game.player.focus = 1;
        game.player.pos = game.towns.home.respawn();
        game.round(Some(Command::Heal));
        assert_eq!((game.player.focus, game.player.max_focus), (20, 20));
        game.player.pos = game.towns.home.shop();
        let gold = game.player.gold;
        game.round(Some(Command::Sell("stillstone".into())));
        assert_eq!(game.player.gold, gold, "a worn relic is not in the sack to sell");
        game.round(Some(Command::Remove("relic".into())));
        game.round(Some(Command::Drop("stillstone".into())));
        game.round(Some(Command::Get("relic".into())));
        game.round(Some(Command::Sell("stillstone".into())));
        assert_eq!(game.player.gold, gold + 75);
        assert!(!game.player.sack.contains(&Item::Relic(Relic::Stillstone)));
        assert_eq!((game.player.focus, game.player.max_focus), (20, 24));
    }

    fn expected_skill(evidence: u32, threat: u32) -> Skill {
        let mut skill = Skill::default();
        skill.learn(evidence, ceiling(threat));
        skill
    }

    fn rng_for_d20(natural: i32) -> Rng {
        (0..10_000)
            .find(|seed| {
                let mut rng = Rng::new(*seed);
                rng.d20() == natural
            })
            .map(Rng::new)
            .expect("a seed for the requested d20 roll")
    }

    fn rng_for_attack(natural: i32, damage: i32) -> Rng {
        (0..100_000)
            .find(|seed| {
                let mut rng = Rng::new(*seed);
                rng.d20() == natural && rng.range(4, 12) == damage
            })
            .map(Rng::new)
            .expect("a seed for the requested attack")
    }

    fn rng_for_heal_and_attack(healing: i32, natural: i32, damage: i32) -> Rng {
        (0..100_000)
            .find(|seed| {
                let mut rng = Rng::new(*seed);
                rng.range(4, 12) == healing && rng.d20() == natural && rng.range(4, 12) == damage
            })
            .map(Rng::new)
            .expect("a seed for the requested healing round")
    }

    fn add_hunting_troll(game: &mut Game, offset: Pos) -> usize {
        let id = game.place(Kind::Troll, (game.player.pos.0 + offset.0, game.player.pos.1 + offset.1));
        let index = game.creatures.iter().position(|creature| creature.id == id).unwrap();
        game.provoke(index);
        index
    }

    fn injury_fixture(seed: u64) -> Game {
        let mut game = made(seed);
        game.player.dex = 3;
        game.player.int = 50;
        game.player.con = 18;
        game.player.armor = None;
        game.player.max_hp = game.player.baseline_max_hp();
        game.player.hp = game.player.max_hp;
        let index = add_hunting_troll(&mut game, (1, 0));
        let threat = game.threat(index, Practice::Restoration);
        assert!(threat >= 10, "the troll is a real Restoration teacher ({threat})");
        let before = game.player.hp;
        game.rng = rng_for_attack(1, 4);
        game.round(Some(Command::Rest));
        assert_eq!(game.player.hp, before - 4);
        assert_eq!(game.player.injuries[0].remaining, 4);
        game
    }

    #[test]
    fn exponential_experience_changes_summary_rank_only() {
        assert_eq!(EXPERIENCE_RANKS, [0, 100, 300, 700, 1_500, 3_100, 6_300, 12_700, 25_500, 51_100]);
        let mut game = made(11);
        game.player.hp = 7;
        game.player.focus = 3;
        let powers =
            (game.player.max_hp, game.player.max_focus, game.player.melee_bonus(), game.player.missile_bonus());
        assert_eq!(game.player.next_level(), Some(100));
        game.gain_xp(99);
        assert_eq!((game.player.level, game.player.next_level()), (1, Some(100)));
        game.gain_xp(1);
        assert_eq!((game.player.level, game.player.next_level()), (2, Some(300)));
        game.gain_xp(51_000);
        assert_eq!(game.player.level, 10);
        assert_eq!(game.player.xp, 51_100);
        assert_eq!(game.player.next_level(), None);
        assert_eq!(game.player.hp, 7);
        assert_eq!(game.player.focus, 3);
        assert_eq!(
            (game.player.max_hp, game.player.max_focus, game.player.melee_bonus(), game.player.missile_bonus(),),
            powers,
            "XP rank grants no combat bonus, capacity, or refill",
        );
        assert!(game.log.iter().any(|line| line.text.contains("adventuring rank 10")));
    }

    #[test]
    fn vitality_and_focus_raise_capacity_without_refilling_current_resources() {
        let mut game = made(12);
        game.player.hp = 5;
        game.player.focus = 4;
        let old_hp = game.player.max_hp;
        game.learn_practice(Practice::Vitality, 1_000, 10);
        assert_eq!(game.player.max_hp, old_hp + 4);
        assert_eq!(game.player.hp, 5);
        game.learn_practice(Practice::Focus, 1_000, 10);
        assert_eq!(game.player.max_focus, 26);
        assert_eq!(game.player.focus, 4);
    }

    #[test]
    fn vitality_is_trained_by_real_incoming_damage_without_a_rank_heal() {
        let mut game = made(12);
        game.player.str = 3;
        game.player.dex = 3;
        game.player.con = 3;
        game.player.max_hp = game.player.baseline_max_hp();
        game.player.hp = game.player.max_hp;
        game.place(Kind::Troll, (game.player.pos.0 + 1, game.player.pos.1));
        game.provoke(0);
        let threat = game.threat(0, Practice::Vitality);
        assert!(threat >= 100, "the troll is a serious Vitality teacher ({threat})");
        let initial_max = game.player.max_hp;
        for _ in 0..3 {
            game.creature_strike(0, 100, (3, 3), None);
        }
        assert_eq!(game.player.practice.get(Practice::Vitality).rank(), Rank::Practiced);
        assert_eq!(game.player.max_hp, initial_max + 4);
        assert_eq!(game.player.hp, initial_max - 9, "growing the cap does not restore Health");
        assert_eq!(game.creatures[0].vitality_lessons_left, game.creatures[0].stats().hp - 9);
    }

    #[test]
    fn weak_creatures_teach_nothing_but_damage_still_uses_the_shared_allowance() {
        let mut game = made(13);
        game.player.str = 18;
        game.player.dex = 18;
        game.player.con = 18;
        game.place(Kind::Rat, (game.player.pos.0 + 1, game.player.pos.1));
        game.place(Kind::Troll, (game.player.pos.0 + 2, game.player.pos.1));
        let threat = game.threat(0, Practice::Blades);
        let stronger_threat = game.threat(1, Practice::Blades);
        assert!(threat < 10, "a rat is harmless practice (threat {threat})");
        assert!(stronger_threat > threat, "stronger threats teach higher ceilings");
        game.wound(0, 3, Practice::Blades, 0);
        assert_eq!(game.player.practice.get(Practice::Blades).rank(), Rank::Novice);
        assert_eq!(game.creatures[0].learning_damage_left, 2);
        assert_eq!(game.creatures[0].stealth_lessons_left, 12);
    }

    #[test]
    fn useful_damage_is_capped_shared_across_magic_and_survives_healing_reload_and_death() {
        let mut game = made(14);
        game.player.int = 30;
        game.place(Kind::Troll, (game.player.pos.0 + 1, game.player.pos.1));
        let full_hp = game.creatures[0].stats().hp;
        let magic_threat = game.threat(0, Practice::Evocation);
        let blade_threat = game.threat(0, Practice::Dagger);
        assert!(magic_threat >= 10 && blade_threat >= 10);

        game.wound(0, 20, Practice::Evocation, 3);
        assert_eq!(game.creatures[0].learning_damage_left, full_hp - 20);
        let mut resumed: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(resumed, game);

        // A troll can heal, but its remaining lesson is still only its original HP minus prior useful damage.
        game.creatures[0].hp = full_hp;
        resumed.creatures[0].hp = full_hp;
        game.wound(0, i32::MAX, Practice::Dagger, 0);
        resumed.wound(0, i32::MAX, Practice::Dagger, 0);
        assert_eq!(resumed, game, "a save/reload replays remaining claims exactly");
        assert!(game.creatures.is_empty());
        assert_eq!(game.player.practice.get(Practice::Evocation), expected_skill(20 * magic_threat, magic_threat),);
        assert_eq!(game.player.practice.get(Practice::Focus), expected_skill(3 * magic_threat, magic_threat),);
        assert_eq!(
            game.player.practice.get(Practice::Dagger),
            expected_skill((full_hp as u32 - 20) * blade_threat, blade_threat),
            "overkill only credits remaining useful damage",
        );

        let learned = game.player.practice;
        game.die();
        let saved: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(saved.player.practice, learned, "death and save preserve practice claims");
    }

    #[test]
    fn weapon_and_evocation_practice_change_landed_damage() {
        let mut novice = made(15);
        novice.player.str = 50;
        novice.player.int = 50;
        novice.place(Kind::Troll, (novice.player.pos.0 + 1, novice.player.pos.1));
        novice.creatures[0].hp = 1_000;
        let mut practiced = novice.clone();
        practiced.learn_practice(Practice::Blades, 1_000, 100);
        novice.attack(Some(Target::Label('A')));
        practiced.attack(Some(Target::Label('A')));
        assert_eq!(novice.creatures[0].hp - practiced.creatures[0].hp, 1, "blades practice adds real damage");

        let mut novice = made(16);
        novice.player.int = 50;
        novice.place(Kind::Troll, (novice.player.pos.0 + 1, novice.player.pos.1));
        novice.creatures[0].hp = 1_000;
        novice.player.chanted = Some(Spell::Spark);
        let mut practiced = novice.clone();
        practiced.learn_practice(Practice::Evocation, 1_000, 100);
        novice.cast(Some(Target::Label('A')));
        practiced.cast(Some(Target::Label('A')));
        assert_eq!(novice.creatures[0].hp - practiced.creatures[0].hp, 1, "Evocation practice adds real spell damage",);
    }

    #[test]
    fn utility_practice_requires_effective_encounters_and_cannot_be_farmed() {
        let mut safe = made(17);
        safe.player.chanted = Some(Spell::Heal);
        safe.round(Some(Command::Cast(None)));
        assert_eq!(safe.player.practice.get(Practice::Restoration).rank(), Rank::Novice);
        assert_eq!(safe.player.practice.get(Practice::Focus).rank(), Rank::Novice);

        let mut game = made(18);
        game.player.dex = 3;
        game.player.int = 50;
        game.player.con = 18;
        game.player.armor = None;
        game.player.max_hp = game.player.baseline_max_hp();
        game.player.hp = game.player.max_hp;
        let index = add_hunting_troll(&mut game, (1, 0));
        let threat = game.threat(index, Practice::Restoration);
        let lessons = game.creatures[index].utility_lessons_left;
        let before = game.player.hp;
        game.rng = rng_for_attack(1, 4);
        game.round(Some(Command::Rest));
        assert_eq!(game.player.hp, before - 4, "combat, not fixture injury, causes the wound");

        game.player.chanted = Some(Spell::Heal);
        game.rng = rng_for_heal_and_attack(4, 13, 4);
        game.round(Some(Command::Cast(None)));
        assert_eq!(game.creatures[index].utility_lessons_left, lessons - 1);
        assert_eq!(game.player.practice.get(Practice::Restoration), expected_skill(4 * threat, threat));
        assert_eq!(game.player.practice.get(Practice::Focus), expected_skill(4 * threat, threat));

        let lessons = game.creatures[index].utility_lessons_left;
        let restoration = game.player.practice.get(Practice::Restoration);
        let focus = game.player.practice.get(Practice::Focus);
        game.player.hp = game.player.max_hp;
        game.player.chanted = Some(Spell::Heal);
        game.round(Some(Command::Cast(None)));
        assert_eq!(game.creatures[index].utility_lessons_left, lessons, "a full-health cast did nothing");
        assert_eq!(game.player.practice.get(Practice::Restoration), restoration);
        assert_eq!(game.player.practice.get(Practice::Focus), focus);
    }

    #[test]
    fn warding_requires_preventing_a_real_hit_and_focus_is_once_per_paid_cast() {
        let mut game = made(23);
        game.player.dex = 20;
        game.player.int = 3;
        game.player.con = 18;
        game.player.armor = Some(Armor::Leather);
        game.player.max_hp = game.player.baseline_max_hp();
        game.player.hp = game.player.max_hp;
        let index = add_hunting_troll(&mut game, (1, 0));
        let ward_threat = game.threat(index, Practice::Warding);
        let focus_threat = game.threat(index, Practice::Focus);
        assert!(ward_threat >= 10 && focus_threat >= 10);
        let lessons = game.creatures[index].utility_lessons_left;

        game.player.chanted = Some(Spell::Shield);
        game.rng = rng_for_d20(10);
        game.round(Some(Command::Cast(None)));
        assert!(game.log.iter().any(|line| line.text.contains("misses you")));
        assert_eq!(game.player.practice.get(Practice::Warding), Skill::default());
        assert_eq!(game.player.practice.get(Practice::Focus), Skill::default());
        assert_eq!(game.creatures[index].utility_lessons_left, lessons);
        assert_eq!(game.player.shield_focus_credit, Spell::Shield.cost() as u32);

        for spent in 1..=2 {
            game.rng = rng_for_d20(13); // 13 + the troll's 6 hits AC 17, but not warded AC 20.
            game.round(Some(Command::Rest));
            assert!(game.log.iter().any(|line| line.text.contains("ward turns aside")));
            assert_eq!(game.creatures[index].utility_lessons_left, lessons - spent);
            assert_eq!(
                game.player.practice.get(Practice::Warding),
                expected_skill(5 * spent as u32 * ward_threat, ward_threat),
            );
            assert_eq!(
                game.player.practice.get(Practice::Focus),
                expected_skill(Spell::Shield.cost() as u32 * focus_threat, focus_threat),
                "later preventions from the same paid cast do not multiply Focus",
            );
        }
        assert_eq!(game.player.shield_focus_credit, 0);

        while game.creatures[index].utility_lessons_left > 0 {
            game.rng = rng_for_d20(13);
            game.round(Some(Command::Rest));
        }
        assert_eq!(game.player.practice.get(Practice::Warding), expected_skill(60 * ward_threat, ward_threat));
        game.rng = rng_for_d20(13);
        game.round(Some(Command::Rest));
        assert_eq!(game.creatures[index].utility_lessons_left, 0, "one troll has a finite prevention lesson budget");
        assert_eq!(
            game.player.practice.get(Practice::Focus),
            expected_skill(Spell::Shield.cost() as u32 * focus_threat, focus_threat),
        );

        let mut hit = made(24);
        hit.player.dex = 20;
        hit.player.int = 3;
        hit.player.con = 18;
        hit.player.armor = Some(Armor::Leather);
        hit.player.max_hp = hit.player.baseline_max_hp();
        hit.player.hp = hit.player.max_hp;
        let index = add_hunting_troll(&mut hit, (1, 0));
        let lessons = hit.creatures[index].utility_lessons_left;
        hit.player.chanted = Some(Spell::Shield);
        hit.rng = rng_for_d20(15); // This roll hits even through the ward.
        hit.round(Some(Command::Cast(None)));
        assert!(hit.log.iter().any(|line| line.text.contains("hits you for")));
        assert_eq!(hit.creatures[index].utility_lessons_left, lessons);
        assert_eq!(hit.player.practice.get(Practice::Warding), Skill::default());
        assert_eq!(hit.player.practice.get(Practice::Focus), Skill::default());
        assert_eq!(hit.player.shield_focus_credit, Spell::Shield.cost() as u32);
    }

    #[test]
    fn effective_stun_teaches_binding_and_focus_only_when_its_duration_grows() {
        let mut game = made(25);
        game.player.int = 50;
        game.player.con = 18;
        game.player.max_hp = game.player.baseline_max_hp();
        game.player.hp = game.player.max_hp;
        let index = add_hunting_troll(&mut game, (1, 0));
        let binding_threat = game.threat(index, Practice::Binding);
        let focus_threat = game.threat(index, Practice::Focus);
        let lessons = game.creatures[index].utility_lessons_left;
        game.player.chanted = Some(Spell::Stun);
        game.rng = rng_for_d20(10);
        game.round(Some(Command::Cast(Some(Target::Label('A')))));
        assert_eq!(game.creatures[index].utility_lessons_left, lessons - 1);
        assert_eq!(game.player.practice.get(Practice::Binding), expected_skill(binding_threat, binding_threat));
        assert_eq!(
            game.player.practice.get(Practice::Focus),
            expected_skill(Spell::Stun.cost() as u32 * focus_threat, focus_threat),
        );
        assert_eq!(game.creatures[index].stunned, 0, "the effective one-round stun elapses in its action");

        let binding = game.player.practice.get(Practice::Binding);
        let focus = game.player.practice.get(Practice::Focus);
        game.creatures[index].stunned = 5;
        game.player.chanted = Some(Spell::Stun);
        game.rng = rng_for_d20(10);
        game.round(Some(Command::Cast(Some(Target::Label('A')))));
        assert_eq!(game.creatures[index].utility_lessons_left, lessons - 1);
        assert_eq!(game.player.practice.get(Practice::Binding), binding);
        assert_eq!(game.player.practice.get(Practice::Focus), focus);
        assert_eq!(game.creatures[index].stunned, 4, "an unchanged duration is not credited");
    }

    #[test]
    fn potion_rest_and_temple_healing_consume_real_injury_provenance() {
        for source in 0..4 {
            let mut game = injury_fixture(30 + source);
            game.clear_creatures();
            match source {
                0 => {
                    game.player.chanted = Some(Spell::Heal);
                    game.rng = rng_for_heal_and_attack(4, 1, 4);
                    game.round(Some(Command::Cast(None)));
                }
                1 => {
                    game.rng = Rng::new(91);
                    game.round(Some(Command::Quaff));
                }
                2 => {
                    game.round(Some(Command::Rest));
                    game.round(Some(Command::Rest));
                }
                _ => {
                    let town = game.level().town_at(game.player.pos).unwrap();
                    game.player.pos = town.respawn();
                    game.round(Some(Command::Heal));
                }
            }
            assert!(
                game.player.injuries.is_empty(),
                "healing source {source} consumes the injury record left by real combat",
            );
            assert_eq!(game.player.practice.get(Practice::Restoration).rank(), Rank::Novice);

            let before = game.player.practice;
            game.player.hp = game.player.max_hp - 4; // Manufactured missing Health has no combat provenance.
            game.player.chanted = Some(Spell::Heal);
            game.rng = rng_for_heal_and_attack(4, 1, 4);
            game.round(Some(Command::Cast(None)));
            assert_eq!(game.player.practice, before, "the same untracked injury cannot be claimed later");
        }
    }

    #[test]
    fn exhausted_utility_lessons_cannot_train_focus_through_new_paid_casts() {
        for spell in [Spell::Shield, Spell::Stun, Spell::Heal] {
            let mut game = injury_fixture(35);
            game.player.dex = 20;
            game.player.int = 20;
            game.player.armor = Some(Armor::Leather);
            game.creatures[0].utility_lessons_left = 0;
            game.player.chanted = Some(spell);
            game.rng = if spell == Spell::Heal { rng_for_heal_and_attack(4, 13, 4) } else { rng_for_d20(13) };
            let before = game.player.practice;
            let focus = game.player.focus;
            game.round(Some(Command::Cast(spell.aimed().then_some(Target::Label('A')))));
            assert!(game.player.focus < focus, "{spell:?} was actually paid for");
            for discipline in [Practice::Warding, Practice::Binding, Practice::Restoration, Practice::Focus] {
                assert_eq!(
                    game.player.practice.get(discipline),
                    before.get(discipline),
                    "{spell:?} cannot reopen a spent utility allowance for {discipline:?}",
                );
            }
            assert_eq!(game.creatures[0].utility_lessons_left, 0);
        }
    }

    #[test]
    fn injury_and_ward_focus_provenance_survive_saving_and_loading() {
        let mut game = injury_fixture(35);
        game.player.dex = 20;
        game.player.armor = Some(Armor::Leather);
        let index = 0;
        let focus_threat = game.threat(index, Practice::Focus);
        game.player.chanted = Some(Spell::Shield);
        game.rng = rng_for_d20(10);
        game.round(Some(Command::Cast(None)));
        assert_eq!(game.player.shield_focus_credit, Spell::Shield.cost() as u32);

        let mut loaded: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(loaded, game);
        game.rng = rng_for_d20(13);
        loaded.rng = game.rng.clone();
        game.round(Some(Command::Rest));
        loaded.round(Some(Command::Rest));
        assert_eq!(loaded, game);
        assert_eq!(game.player.shield_focus_credit, 0);
        assert_eq!(
            game.player.practice.get(Practice::Focus),
            expected_skill(Spell::Shield.cost() as u32 * focus_threat, focus_threat),
        );

        let restoration_threat = game.player.injuries[0].threat;
        let lessons = game.creatures[index].utility_lessons_left;
        game.player.chanted = Some(Spell::Heal);
        loaded.player.chanted = Some(Spell::Heal);
        game.rng = rng_for_heal_and_attack(4, 13, 4);
        loaded.rng = game.rng.clone();
        game.round(Some(Command::Cast(None)));
        loaded.round(Some(Command::Cast(None)));
        assert_eq!(loaded, game);
        assert_eq!(game.player.injuries[0].remaining, 0, "the loaded injury source is consumed by Heal");
        assert_eq!(
            game.creatures[index].utility_lessons_left,
            lessons - 2,
            "the heal and the ward's next prevention are separate utility lessons",
        );
        assert_eq!(
            game.player.practice.get(Practice::Restoration),
            expected_skill(4 * restoration_threat, restoration_threat),
        );
    }

    #[test]
    fn stealth_practice_uses_unseen_movement_openings_and_finite_escapes() {
        let mut movement = made(20);
        movement.player.dex = 50;
        movement.player.hidden = true;
        movement.place(Kind::Troll, (movement.player.pos.0 + 4, movement.player.pos.1));
        let threat = movement.threat(0, Practice::Stealth);
        assert!(threat >= 10);
        movement.learn_unseen_movement();
        assert_eq!(movement.creatures[0].stealth_lessons_left, 11);
        assert_eq!(movement.player.practice.get(Practice::Stealth), expected_skill(2 * threat, threat),);

        let mut opening = made(21);
        opening.player.str = 50;
        opening.player.hidden = true;
        let id = opening.place(Kind::Troll, (opening.player.pos.0 + 1, opening.player.pos.1));
        opening.creatures[0].hp = 1_000;
        let threat = opening.threat(0, Practice::Stealth);
        opening.attack(Some(Target::Label('A')));
        assert_eq!(opening.creatures[0].stealth_lessons_left, 9);
        assert!(opening.creatures[0].revealed_by_attack);
        assert_eq!(opening.player.practice.get(Practice::Stealth), expected_skill(6 * threat, threat),);

        opening.provoke(0);
        opening.creatures[0].hunting = false;
        opening.learn_escapes(&[id]);
        assert_eq!(opening.creatures[0].stealth_lessons_left, 9, "an attack reveal is not an escape");

        let mut escape = made(22);
        let id = escape.place(Kind::Troll, (escape.player.pos.0 + 4, escape.player.pos.1));
        let threat = escape.threat(0, Practice::Stealth);
        escape.provoke(0);
        escape.creatures[0].hunting = false;
        for expected_left in [9, 6, 3, 0, 0] {
            escape.learn_escapes(&[id]);
            assert_eq!(escape.creatures[0].stealth_lessons_left, expected_left);
        }
        assert_eq!(
            escape.player.practice.get(Practice::Stealth),
            expected_skill(12 * 2 * threat, threat),
            "one individual pays at most twelve shared lessons",
        );
    }

    #[test]
    fn ordinary_waiting_does_not_regenerate_health_or_focus() {
        let mut game = made(19);
        game.player.hp -= 5;
        game.player.focus -= 5;
        let before = (game.player.hp, game.player.focus);
        for _ in 0..10 {
            game.round(None);
        }
        assert_eq!((game.player.hp, game.player.focus), before);
        game.player.hp = game.player.max_hp - 10;
        game.player.focus = game.player.max_focus - 10;
        let before = (game.player.hp, game.player.focus);
        game.round(Some(Command::Rest));
        assert_eq!(game.player.hp, before.0 + 2);
        assert_eq!(game.player.focus, before.1 + 6);
    }
}
