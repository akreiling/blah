//! The game: a character, the creatures, what lies on the ground, and the round that resolves a command.

use serde::{Deserialize, Serialize};

use crate::command::{self, Command, Dir, Instant, Parsed, Target};
use crate::data::{Armor, Bow, Class, Item, Kind, LEVELS, SHOP, Spell, Weapon, sale_price};
use crate::map::{Level, LevelId, Pos, SpawnKind, Tile, Zone, distance};
use crate::rng::Rng;

/// Cells a character or creature sees.
pub const SIGHT: i32 = 6;
/// Cells a creature notices you from.
const NOTICE: i32 = 7;
/// Rounds a creature keeps hunting after losing sight of you.
const PATIENCE: u32 = 12;
/// How far from home woods beasts follow you; the town lies beyond it.
const WILD_LEASH: i32 = 7;
/// How far from home undercroft creatures follow you.
const DEEP_LEASH: i32 = 14;
/// Rounds before an emptied spawn point raises another creature.
const RESPAWN: u64 = 60;
/// Rounds before the troll returns.
const LAIR_RESPAWN: u64 = 400;
/// Things the sack holds.
pub const SACK: usize = 12;
/// Log lines kept.
const LOG_KEEP: usize = 300;

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
    /// Gains: gold, levels, healing.
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
    /// Waiting for a calling.
    Calling(String),
    /// Playing.
    Play,
}

/// The character.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    /// What they are called.
    pub name: String,
    /// Their calling.
    pub class: Class,
    /// Experience level, from 1.
    pub level: u32,
    /// Experience earned.
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
    /// Mana now.
    pub mp: i32,
    /// Mana at full.
    pub max_mp: i32,
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
    /// Rounds the wizard's shield has left.
    #[serde(default)]
    pub shield: u32,
}

/// The bonus a stat gives.
pub const fn modifier(stat: i32) -> i32 {
    (stat - 10).div_euclid(2)
}

impl Player {
    fn roll(name: String, class: Class, rng: &mut Rng) -> Player {
        let mut stat = || {
            let mut dice = [rng.range(1, 6), rng.range(1, 6), rng.range(1, 6), rng.range(1, 6)];
            dice.sort_unstable();
            dice[1] + dice[2] + dice[3]
        };
        let (mut str, mut dex, mut int, mut con) = (stat(), stat(), stat(), stat());
        match class {
            Class::Fighter => {
                str += 2;
                con += 1;
            }
            Class::Thief => dex += 2,
            Class::Wizard => int += 2,
            Class::Thaumaturge => {
                int += 1;
                con += 1;
            }
        }
        let max_hp = (class.base_hp() + modifier(con)).max(6);
        let max_mp = match class {
            Class::Wizard => 10 + modifier(int).max(0) * 2,
            Class::Thaumaturge => 8 + modifier(int).max(0) * 2,
            _ => 0,
        };
        let (weapon, bow, arrows, armor, gold) = match class {
            Class::Fighter => (Weapon::ShortSword, None, 0, Some(Armor::Leather), 50),
            Class::Thief => (Weapon::Dagger, Some(Bow::Shortbow), 12, Some(Armor::Leather), 30),
            Class::Wizard => (Weapon::Staff, None, 0, None, 40),
            Class::Thaumaturge => (Weapon::Mace, None, 0, Some(Armor::Leather), 30),
        };
        let surface = Level::get(LevelId::Surface);
        Player {
            name,
            class,
            level: 1,
            xp: 0,
            str,
            dex,
            int,
            con,
            hp: max_hp,
            max_hp,
            mp: max_mp,
            max_mp,
            at: LevelId::Surface,
            pos: surface.start.expect("the surface has a start"),
            weapon,
            bow,
            arrows,
            armor,
            ring: false,
            sack: vec![Item::Potion],
            gold,
            nocked: false,
            chanted: None,
            hidden: false,
            deaths: 0,
            shield: 0,
        }
    }

    /// What an attack roll must reach to hit you.
    pub fn ac(&self) -> i32 {
        10 + modifier(self.dex) + self.armor.map_or(0, Armor::protection) + if self.ring { 2 } else { 0 }
    }

    fn melee_bonus(&self) -> i32 {
        self.class.melee_bonus() + self.level as i32 / 2 + modifier(self.str) + self.weapon.accuracy()
    }

    fn missile_bonus(&self) -> i32 {
        self.level as i32 / 2 + modifier(self.dex) + i32::from(self.class == Class::Thief)
    }

    /// Experience needed for the next level, if there is one.
    pub fn next_level(&self) -> Option<u32> {
        LEVELS.get(self.level as usize).copied()
    }
}

/// A creature on the island.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Creature {
    /// Stable identity.
    pub id: u32,
    /// What it is.
    pub kind: Kind,
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
    /// Where it last saw you.
    pub last_seen: Option<Pos>,
    /// Rounds since it last saw you.
    pub lost: u32,
    /// Rounds it stays stunned.
    #[serde(default)]
    pub stunned: u32,
    spawn: Option<usize>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SpawnState {
    at: LevelId,
    index: usize,
    occupant: Option<u32>,
    ready: u64,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    seed: u64,
    rng: Rng,
    round: u64,
    phase: Phase,
    player: Player,
    creatures: Vec<Creature>,
    next_id: u32,
    piles: Vec<Pile>,
    spawns: Vec<SpawnState>,
    /// Per level, cells ever seen.
    seen: Vec<Vec<bool>>,
    log: Vec<Line>,
    log_total: u64,
}

impl Game {
    /// A fresh island for `seed`, waiting for a character's name.
    pub fn new(seed: u64) -> Game {
        let mut rng = Rng::new(seed);
        let player = Player::roll(String::new(), Class::Fighter, &mut rng);
        let mut spawns = Vec::new();
        for id in LevelId::ALL {
            for index in 0..Level::get(id).spawns.len() {
                spawns.push(SpawnState { at: id, index, occupant: None, ready: 0 });
            }
        }
        let seen = LevelId::ALL.iter().map(|&id| vec![false; Level::get(id).cells()]).collect();
        let mut game = Game {
            seed,
            rng,
            round: 0,
            phase: Phase::Name,
            player,
            creatures: Vec::new(),
            next_id: 1,
            piles: Vec::new(),
            spawns,
            seen,
            log: Vec::new(),
            log_total: 0,
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
    pub fn level(&self) -> &'static Level {
        Level::get(self.player.at)
    }

    /// Every creature alive.
    pub fn creatures(&self) -> &[Creature] {
        &self.creatures
    }

    /// The pile on `pos` of the character's map, if any.
    pub fn pile_at(&self, pos: Pos) -> Option<&Pile> {
        self.piles.iter().find(|p| p.at == self.player.at && p.pos == pos)
    }

    /// Whether the character has ever seen `pos` on their map.
    pub fn remembers(&self, pos: Pos) -> bool {
        self.level().cell_index(pos).is_some_and(|i| self.seen[self.player.at.index()][i])
    }

    /// Whether the character sees `pos` now.
    pub fn sees(&self, pos: Pos) -> bool {
        self.level().sees(self.player.pos, pos, SIGHT)
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
                self.say(Tone::System, format!("{name}, choose your calling: fighter, thief, thaumaturge, or wizard."));
                self.phase = Phase::Calling(name);
            }
            Phase::Calling(name) => {
                let typed = line.to_lowercase();
                let matches: Vec<Class> =
                    Class::ALL.into_iter().filter(|c| !typed.is_empty() && c.name().starts_with(&typed)).collect();
                let [class] = matches[..] else {
                    self.say(Tone::System, "Fighter, thief, thaumaturge, or wizard?");
                    return;
                };
                self.player = Player::roll(name, class, &mut self.rng);
                self.phase = Phase::Play;
                let p = &self.player;
                let line = format!(
                    "{} the {}: Str {} Dex {} Int {} Con {}, {} health.",
                    p.name,
                    class.name(),
                    p.str,
                    p.dex,
                    p.int,
                    p.con,
                    p.max_hp
                );
                self.say(Tone::Gain, line);
                self.say(Tone::Info, "You stand in Harrowmere. The outfitter is northwest and the temple northeast.");
                self.say(Tone::Info, "South, past the woods, the old watchtower's stairs lead down.");
                self.say(Tone::System, "Type help for the commands. Each command you enter takes one round.");
                self.spawn_all();
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
            Instant::Look => {
                let place = match self.zone() {
                    Zone::Shop => "the outfitter's shop".to_owned(),
                    Zone::Temple => "the temple".to_owned(),
                    Zone::Open => self.level().name.to_owned(),
                };
                let underfoot = match self.level().tile(self.player.pos) {
                    Tile::StairsDown => " Stairs lead down.",
                    Tile::StairsUp => " Stairs lead up.",
                    _ => "",
                };
                self.say(Tone::Info, format!("You are in {place}.{underfoot}"));
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
                        format!("{} {} ({} {way})", s.label, s.creature.kind.stats().name, distance((0, 0), (dx, dy)))
                    })
                    .collect();
                if !sighted.is_empty() {
                    self.say(Tone::Info, format!("You see: {}.", sighted.join(", ")));
                }
            }
        }
    }

    // ----- The round -----

    /// Resolves one round: your command if any, then every creature, then healing and new arrivals.
    pub fn round(&mut self, command: Option<Command>) {
        if self.phase != Phase::Play {
            return;
        }
        self.round += 1;
        match command {
            Some(command) => self.perform(command),
            None => self.stand_ready(),
        }
        self.creatures_act();
        if self.player.hp <= 0 {
            self.die();
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
            Command::Quaff => self.quaff(),
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
        for (i, dir) in steps.iter().enumerate() {
            let (dx, dy) = dir.delta();
            let next = (self.player.pos.0 + dx, self.player.pos.1 + dy);
            if let Some(c) = self.creature_at(next) {
                let name = c.kind.stats().name;
                self.say(Tone::Info, format!("A {name} is in the way."));
                return;
            }
            if !self.level().tile(next).walkable() {
                self.say(Tone::Info, "You can't go that way.");
                return;
            }
            self.player.pos = next;
            if self.player.hidden && !self.rng.chance(70 + modifier(self.player.dex) * 5) {
                self.player.hidden = false;
                self.say(Tone::Info, "You step out of the shadows.");
            }
            if let Some(pile) = self.pile_at(next) {
                let items: Vec<String> = pile.items.iter().map(|i| i.name()).collect();
                self.say(Tone::Info, format!("You see here: {}.", items.join(", ")));
            }
            if i + 1 < steps.len()
                && let Some(c) = self.hunter_adjacent()
            {
                let name = c.kind.stats().name;
                self.say(Tone::Info, format!("You stop: a {name} is beside you."));
                return;
            }
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
                    let words: Vec<&str> = s.creature.kind.stats().name.split(' ').collect();
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
        let name = self.creatures[index].kind.stats().name;
        if distance(self.creatures[index].pos, self.player.pos) > 1 {
            self.say(Tone::Info, format!("The {name} is out of reach."));
            return;
        }
        self.player.nocked = false;
        let ambush = std::mem::take(&mut self.player.hidden);
        let ac = self.creatures[index].kind.stats().ac;
        let roll = self.rng.d20() + self.player.melee_bonus() + if ambush { 4 } else { 0 };
        if roll < ac && roll - self.player.melee_bonus() != 20 {
            self.say(Tone::Miss, format!("You miss the {name}."));
            self.provoke(index);
            return;
        }
        let (lo, hi) = self.player.weapon.damage();
        let mut damage = (self.rng.range(lo, hi) + modifier(self.player.str)).max(1);
        if ambush {
            damage *= 3;
            self.say(Tone::Hit, format!("You strike the {name} from the shadows for {damage}!"));
        } else {
            self.say(Tone::Hit, format!("You hit the {name} for {damage}."));
        }
        self.wound(index, damage);
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
        let name = self.creatures[index].kind.stats().name;
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
        let stats = self.creatures[index].kind.stats();
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
        let mut damage = (self.rng.range(lo, hi) + modifier(self.player.dex).max(0)).max(1);
        if ambush {
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
        self.wound(index, damage);
    }

    fn chant(&mut self, spell: Spell) {
        if self.player.class != spell.class() {
            self.say(Tone::Info, format!("Only a {} knows the words of {}.", spell.class().name(), spell.name()));
            return;
        }
        if self.player.class == Class::Wizard && self.player.armor.is_some_and(Armor::metal) {
            self.say(Tone::Info, "Your metal armor smothers the words. Wizards cast in leather or cloth.");
            return;
        }
        self.player.chanted = Some(spell);
        self.say(Tone::Info, format!("You chant the words of {}.", spell.name()));
    }

    fn cast(&mut self, target: Option<Target>) {
        let Some(spell) = self.player.chanted else {
            self.say(Tone::Info, "You have no spell chanted.");
            return;
        };
        if self.player.mp < spell.cost() {
            self.say(Tone::Info, format!("You lack the mana for {} ({} needed).", spell.name(), spell.cost()));
            return;
        }
        if !spell.aimed() {
            self.player.mp -= spell.cost();
            self.player.chanted = None;
            if spell == Spell::Heal {
                let healed = self.rng.range(4, 12) + 2 * self.player.level as i32;
                self.player.hp = (self.player.hp + healed).min(self.player.max_hp);
                self.say(Tone::Gain, format!("Warmth knits your wounds. (+{healed} health)"));
            } else {
                self.player.shield = 20 + 2 * self.player.level;
                self.say(Tone::Gain, "A shimmering ward settles around you.");
            }
            return;
        }
        let Some(index) = self.find_target(target) else { return };
        if !self.in_line(index, SIGHT) {
            return;
        }
        self.player.mp -= spell.cost();
        self.player.chanted = None;
        self.player.hidden = false;
        let stats = self.creatures[index].kind.stats();
        let natural = self.rng.d20();
        let roll = natural + self.player.level as i32 / 2 + modifier(self.player.int) + 2;
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
            // The troll is too big to hold for long.
            let rounds = if self.creatures[index].kind == Kind::Troll { 1 } else { self.rng.range(2, 3) as u32 };
            self.creatures[index].stunned = rounds;
            self.say(Tone::Hit, format!("The {} reels, stunned!", stats.name));
            self.provoke(index);
            return;
        }
        let damage = (self.rng.range(2, 8) + modifier(self.player.int).max(0) + self.player.level as i32 / 2).max(1);
        self.say(Tone::Hit, format!("A spark leaps from your hand and burns the {} for {damage}!", stats.name));
        self.wound(index, damage);
    }

    fn hide(&mut self) {
        if self.player.class != Class::Thief {
            self.say(Tone::Info, "You have no knack for hiding.");
            return;
        }
        if let Some(c) = self.hunter_adjacent() {
            let name = c.kind.stats().name;
            self.say(Tone::Info, format!("The {name} is watching you too closely."));
            return;
        }
        let watchers = self.creatures.iter().filter(|c| c.at == self.player.at && c.hunting).count() as i32;
        if self.rng.chance(60 + modifier(self.player.dex) * 5 - watchers * 15) {
            self.player.hidden = true;
            for c in self.creatures.iter_mut().filter(|c| c.at == self.player.at) {
                c.hunting = false;
            }
            self.say(Tone::Info, "You melt into the shadows.");
        } else {
            self.say(Tone::Info, "You fail to find any cover.");
        }
    }

    /// Hurts the creature at `index`, killing it at zero health.
    fn wound(&mut self, index: usize, damage: i32) {
        self.creatures[index].hp -= damage;
        if self.creatures[index].hp > 0 {
            self.provoke(index);
            return;
        }
        let dead = self.creatures.remove(index);
        let stats = dead.kind.stats();
        self.say(Tone::Kill, format!("The {} dies! (+{} experience)", stats.name, stats.xp));
        if let Some(spawn) = dead.spawn {
            let delay = if dead.kind == Kind::Troll { LAIR_RESPAWN } else { RESPAWN };
            self.spawns[spawn].occupant = None;
            self.spawns[spawn].ready = self.round + delay;
        }
        let gold = self.rng.range(stats.gold.0, stats.gold.1);
        if gold > 0 {
            self.drop_at(dead.at, dead.pos, Item::Gold(gold as u32));
        }
        for item in self.loot(dead.kind) {
            self.drop_at(dead.at, dead.pos, item);
        }
        self.gain_xp(stats.xp);
    }

    fn loot(&mut self, kind: Kind) -> Vec<Item> {
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
    }

    fn gain_xp(&mut self, xp: u32) {
        self.player.xp += xp;
        while let Some(needed) = self.player.next_level()
            && self.player.xp >= needed
        {
            let p = &mut self.player;
            p.level += 1;
            let more = (p.class.hp_per_level() + modifier(p.con)).max(1);
            p.max_hp += more;
            p.hp += more;
            if p.class == Class::Wizard {
                p.max_mp += 4;
                p.mp += 4;
            }
            let level = p.level;
            self.say(Tone::Gain, format!("You feel more capable. You are now level {level}!"));
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
        let (at, pos) = (self.player.at, self.player.pos);
        let Some(index) = self.piles.iter().position(|p| p.at == at && p.pos == pos) else {
            self.say(Tone::Info, "There is nothing here.");
            return;
        };
        let wanted: Vec<usize> = if what == "all" {
            (0..self.piles[index].items.len()).collect()
        } else {
            self.piles[index].items.iter().position(|i| i.matches(what)).into_iter().collect()
        };
        if wanted.is_empty() {
            self.say(Tone::Info, format!("You see no {what} here."));
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

    fn quaff(&mut self) {
        let Some(i) = self.player.sack.iter().position(|&i| i == Item::Potion) else {
            self.say(Tone::Info, "You have no potion.");
            return;
        };
        self.player.sack.remove(i);
        let healed = self.rng.range(10, 16) + 2 * self.player.level as i32;
        self.player.hp = (self.player.hp + healed).min(self.player.max_hp);
        self.say(Tone::Gain, format!("You drink the potion. (+{healed} health)"));
    }

    fn rest(&mut self) {
        if let Some(c) = self.creatures.iter().find(|c| c.at == self.player.at && c.hunting) {
            let name = c.kind.stats().name;
            self.say(Tone::Info, format!("You can't rest with a {name} hunting you."));
            return;
        }
        let p = &mut self.player;
        p.hp = (p.hp + 2).min(p.max_hp);
        p.mp = (p.mp + 2).min(p.max_mp);
        self.say(Tone::Info, "You rest a while.");
    }

    fn stairs(&mut self, down: bool) {
        let tile = self.level().tile(self.player.pos);
        let destination = match (down, tile, self.player.at) {
            (true, Tile::StairsDown, LevelId::Surface) => LevelId::Undercroft,
            (false, Tile::StairsUp, LevelId::Undercroft) => LevelId::Surface,
            _ => {
                let way = if down { "down" } else { "up" };
                self.say(Tone::Info, format!("There are no stairs {way} here."));
                return;
            }
        };
        let level = Level::get(destination);
        let arrive = if down { level.stairs_up } else { level.stairs_down };
        self.player.at = destination;
        self.player.pos = arrive.expect("both ends of the stairs exist");
        self.player.hidden = false;
        let text = if down {
            "You descend into the Undercroft. The air is cold and smells of old stone."
        } else {
            "You climb back into the daylight of Harrowmere."
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
        let p = &mut self.player;
        if p.hp == p.max_hp && p.mp == p.max_mp {
            self.say(Tone::Info, "\"You are whole already,\" the priest says.");
            return;
        }
        let cost = (p.level * 5).min(p.gold);
        p.gold -= cost;
        p.hp = p.max_hp;
        p.mp = p.max_mp;
        self.say(Tone::Gain, format!("The priest lays hands on you. You are healed. ({cost} gold)"));
    }

    // ----- Creatures -----

    fn creatures_act(&mut self) {
        let ids: Vec<u32> = self.creatures.iter().filter(|c| c.at == self.player.at).map(|c| c.id).collect();
        for id in ids {
            if self.player.hp <= 0 {
                return;
            }
            let Some(index) = self.creatures.iter().position(|c| c.id == id) else { continue };
            self.creature_turn(index);
        }
    }

    fn creature_turn(&mut self, index: usize) {
        let level = self.level();
        let me = self.player.pos;
        let (pos, kind) = (self.creatures[index].pos, self.creatures[index].kind);
        let stats = kind.stats();
        if self.creatures[index].stunned > 0 {
            self.creatures[index].stunned -= 1;
            return;
        }
        let close = distance(pos, me) <= 1;
        let leash = if self.player.at == LevelId::Surface { WILD_LEASH } else { DEEP_LEASH };
        let home = self.creatures[index].home;
        let in_reach = distance(me, home) <= leash + 1;
        let in_sight = in_reach && level.sees(pos, me, NOTICE);
        let notices = in_sight && (!self.player.hidden || (close && self.rng.chance(30)));
        if !in_reach && self.creatures[index].hunting {
            let c = &mut self.creatures[index];
            c.hunting = false;
            c.last_seen = None;
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
        } else if self.creatures[index].hunting {
            let c = &mut self.creatures[index];
            c.lost += 1;
            if c.lost > PATIENCE {
                c.hunting = false;
                c.last_seen = None;
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
            let goal = c.last_seen.unwrap_or(me);
            self.creature_step(index, goal);
        } else if distance(pos, home) > 4 {
            self.creature_step(index, home);
        } else if self.rng.chance(30) {
            let dir = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw][self.rng.below(8) as usize];
            let (dx, dy) = dir.delta();
            let next = (pos.0 + dx, pos.1 + dy);
            if distance(next, self.creatures[index].home) <= 4 && self.open_for_creature(next) {
                self.creatures[index].pos = next;
            }
        }
    }

    fn creature_strike(&mut self, index: usize, hit: i32, (lo, hi): (i32, i32), missile: Option<&str>) {
        let name = self.creatures[index].kind.stats().name;
        let natural = self.rng.d20();
        let ward = match (self.player.shield > 0, missile.is_some()) {
            (true, true) => 6,
            (true, false) => 3,
            (false, _) => 0,
        };
        if natural + hit < self.player.ac() + ward && natural != 20 {
            let text = match missile {
                Some(m) => format!("The {name}'s {m} whistles past you."),
                None => format!("The {name} misses you."),
            };
            self.say(Tone::Miss, text);
            return;
        }
        let damage = self.rng.range(lo, hi);
        self.player.hp -= damage;
        let text = match missile {
            Some(m) => format!("The {name}'s {m} hits you for {damage}!"),
            None => format!("The {name} hits you for {damage}!"),
        };
        self.say(Tone::Hurt, text);
    }

    fn open_for_creature(&self, pos: Pos) -> bool {
        let level = self.level();
        level.tile(pos).walkable()
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
            .filter(|&p| self.open_for_creature(p) && distance(p, home) <= leash.max(distance(pos, home)))
            .min_by_key(|&p| score(p));
        if let Some(next) = best
            && score(next) < score(pos)
        {
            self.creatures[index].pos = next;
        } else if Some(pos) == self.creatures[index].last_seen || best.is_none() {
            self.creatures[index].last_seen = None;
        }
    }

    fn die(&mut self) {
        let p = &mut self.player;
        p.deaths += 1;
        let (at, pos, gold) = (p.at, p.pos, std::mem::take(&mut p.gold));
        p.hp = p.max_hp;
        p.mp = p.max_mp;
        p.nocked = false;
        p.chanted = None;
        p.hidden = false;
        p.at = LevelId::Surface;
        p.pos = Level::get(LevelId::Surface).respawn.expect("the temple has a resting place");
        if gold > 0 {
            self.drop_at(at, pos, Item::Gold(gold));
        }
        for c in &mut self.creatures {
            c.hunting = false;
            c.last_seen = None;
        }
        self.say(Tone::Death, "You have died!");
        self.say(Tone::System, "You wake on the temple floor. Your gold lies where you fell.");
    }

    fn recover(&mut self) {
        let p = &mut self.player;
        if p.shield > 0 {
            p.shield -= 1;
            if p.shield == 0 {
                self.say(Tone::Info, "Your ward fades.");
            }
        }
        let p = &mut self.player;
        if self.round.is_multiple_of(5) {
            p.hp = (p.hp + 1).min(p.max_hp);
        }
        if self.round.is_multiple_of(3) {
            p.mp = (p.mp + 1).min(p.max_mp);
        }
    }

    fn spawn_all(&mut self) {
        for i in 0..self.spawns.len() {
            self.spawn(i);
        }
    }

    fn respawn(&mut self) {
        for i in 0..self.spawns.len() {
            let s = &self.spawns[i];
            if s.occupant.is_some() || self.round < s.ready {
                continue;
            }
            let at = Level::get(s.at).spawns[s.index].0;
            if s.at == self.player.at && distance(at, self.player.pos) <= SIGHT + 2 {
                continue;
            }
            self.spawn(i);
        }
    }

    fn spawn(&mut self, i: usize) {
        let s = &self.spawns[i];
        let (pos, kind) = Level::get(s.at).spawns[s.index];
        let at = s.at;
        let kind = match kind {
            SpawnKind::Lair => Kind::Troll,
            SpawnKind::Wild => [Kind::Wolf, Kind::Wolf, Kind::Rat][self.rng.below(3) as usize],
            SpawnKind::Deep => {
                const DEEP: [(Kind, u32); 5] = [
                    (Kind::Rat, 25),
                    (Kind::Goblin, 25),
                    (Kind::GoblinArcher, 15),
                    (Kind::Orc, 20),
                    (Kind::Skeleton, 15),
                ];
                let mut roll = self.rng.below(100);
                DEEP.iter()
                    .find(|&&(_, w)| {
                        let hit = roll < w;
                        roll = roll.saturating_sub(w);
                        hit
                    })
                    .map_or(Kind::Rat, |&(k, _)| k)
            }
        };
        let id = self.next_id;
        self.next_id += 1;
        self.creatures.push(Creature {
            id,
            kind,
            at,
            pos,
            hp: kind.stats().hp,
            home: pos,
            hunting: false,
            last_seen: None,
            lost: 0,
            stunned: 0,
            spawn: Some(i),
        });
        self.spawns[i].occupant = Some(id);
    }

    fn update_seen(&mut self) {
        let level = self.level();
        let me = self.player.pos;
        let seen = &mut self.seen[self.player.at.index()];
        for y in me.1 - SIGHT..=me.1 + SIGHT {
            for x in me.0 - SIGHT..=me.0 + SIGHT {
                if let Some(i) = level.cell_index((x, y))
                    && level.sees(me, (x, y), SIGHT)
                {
                    seen[i] = true;
                }
            }
        }
    }
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
        self.creatures.push(Creature {
            id,
            kind,
            at,
            pos,
            hp: kind.stats().hp,
            home: pos,
            hunting: false,
            last_seen: None,
            lost: 0,
            stunned: 0,
            spawn: None,
        });
        id
    }

    pub(crate) fn clear_creatures(&mut self) {
        self.creatures.clear();
        for s in &mut self.spawns {
            s.occupant = Some(u32::MAX);
        }
    }
}
