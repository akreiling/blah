use std::collections::{BTreeMap, BTreeSet, VecDeque};

use isle_core::{
    Armor, Command, Dir, Game, Item, LevelId, MAX_STEPS, Player, Pos, Relic, SIGHT, Spell, Target, Tile, Town, Weapon,
    distance,
};

const DIRS: [Dir; 8] = [Dir::N, Dir::S, Dir::E, Dir::W, Dir::Ne, Dir::Nw, Dir::Se, Dir::Sw];

#[derive(Default)]
struct Memory {
    tiles: BTreeMap<Pos, Tile>,
    visits: BTreeMap<Pos, u32>,
    loot: BTreeSet<Pos>,
}

/// Whether the last moves alternate between exactly two cells, the signature of a goal flipping.
pub fn bouncing(trail: &VecDeque<(LevelId, Pos)>) -> bool {
    let cells: BTreeSet<_> = trail.iter().collect();
    trail.len() >= 40 && cells.len() == 2 && trail.iter().zip(trail.iter().skip(1)).all(|(a, b)| a != b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_never_use_unknown_tiles_or_visible_occupied_cells() {
        let mut memory = Memory::default();
        for pos in [(0, 0), (1, 0), (2, 0)] {
            memory.tiles.insert(pos, Tile::Floor);
        }
        assert_eq!(path(&memory, (0, 0), &BTreeSet::new(), |at| at == (2, 0)), Some(vec![Dir::E, Dir::E]));
        assert!(path(&memory, (0, 0), &BTreeSet::from([(1, 0)]), |at| at == (2, 0)).is_none());
        memory.tiles.remove(&(1, 0));
        assert!(path(&memory, (0, 0), &BTreeSet::new(), |at| at == (2, 0)).is_none());
    }

    #[test]
    fn formerly_stalled_seeds_never_bounce_between_two_cells() {
        // Each seed once alternated forever as a creature entered and left sight.
        for seed in [3, 6, 11, 22] {
            let mut game = Game::new(seed);
            game.answer("Wren");
            let mut pilot = Pilot::new(&game, false);
            let mut trail = VecDeque::new();
            for _ in 0..20_000 {
                game.round(Some(pilot.choose(&game)));
                trail.push_back((game.player().at, game.player().pos));
                if trail.len() > 40 {
                    trail.pop_front();
                }
                assert!(!bouncing(&trail), "seed {seed} round {}: {trail:?}", game.round_number());
            }
        }
    }
}

pub struct Pilot {
    home: Town,
    prepared: bool,
    memory: BTreeMap<LevelId, Memory>,
    retreating: bool,
    refuge: Option<Pos>,
    quarry: Option<(u32, Pos)>,
    goal: Option<(LevelId, Pos)>,
    pub retreats: u64,
}

fn armor_value(armor: Option<Armor>) -> i32 {
    armor.map_or(0, Armor::protection)
}

fn relic_value(relic: Option<Relic>, prepared: bool) -> i32 {
    match relic {
        Some(Relic::NightCrown) => 6,
        Some(Relic::Deepheart) => 5,
        Some(Relic::GraveSeal) if prepared => 5,
        Some(Relic::Stillstone) if prepared => 4,
        Some(Relic::IronrootBead) => 3,
        Some(Relic::RedTooth) => 2,
        _ => 0,
    }
}

fn useful(item: Item, p: &Player, prepared: bool) -> bool {
    match item {
        Item::Gold(_) => true,
        Item::Potion => p.sack.iter().filter(|&&i| i == Item::Potion).count() < 6,
        Item::Weapon(weapon) => weapon.damage().0 + weapon.damage().1 > p.weapon.damage().0 + p.weapon.damage().1,
        Item::Armor(armor) => armor.protection() > armor_value(p.armor),
        Item::Ring => !p.ring,
        Item::Relic(relic) => relic_value(Some(relic), prepared) > relic_value(p.relic, prepared),
        _ => false,
    }
}

fn item_query(item: Item) -> String {
    if matches!(item, Item::Gold(_)) { "gold".into() } else { item.name() }
}

/// A known-terrain path, never through unknown tiles or currently visible creatures.
fn path(memory: &Memory, start: Pos, blocked: &BTreeSet<Pos>, goal: impl Fn(Pos) -> bool) -> Option<Vec<Dir>> {
    let mut queue = VecDeque::from([start]);
    let mut previous = BTreeMap::from([(start, None)]);
    while let Some(at) = queue.pop_front() {
        if goal(at) {
            let mut route = Vec::new();
            let mut here = at;
            while let Some((from, dir)) = previous[&here] {
                route.push(dir);
                here = from;
            }
            route.reverse();
            return Some(route);
        }
        for dir in DIRS {
            let delta = dir.delta();
            let next = (at.0 + delta.0, at.1 + delta.1);
            if blocked.contains(&next)
                || previous.contains_key(&next)
                || !memory.tiles.get(&next).is_some_and(|t| t.walkable())
            {
                continue;
            }
            previous.insert(next, Some((at, dir)));
            queue.push_back(next);
        }
    }
    None
}

/// Moves toward a goal; when only visible creatures stand in the way, walks up to the first one
/// and attacks it if it blocks the very next step. Waiting for blockers to leave can loop forever.
fn approach(
    memory: &Memory,
    start: Pos,
    seen: &[isle_core::Sighted<'_>],
    goal: impl Fn(Pos) -> bool,
) -> Option<Command> {
    let blocked: BTreeSet<_> = seen.iter().map(|s| s.creature.pos).collect();
    if let Some(route) = path(memory, start, &blocked, &goal) {
        return movement(route);
    }
    let route = path(memory, start, &BTreeSet::new(), &goal)?;
    let mut at = start;
    let mut clear = Vec::new();
    for dir in route {
        let (dx, dy) = dir.delta();
        let next = (at.0 + dx, at.1 + dy);
        if let Some(s) = seen.iter().find(|s| s.creature.pos == next) {
            return movement(clear).or(Some(Command::Attack(Some(Target::Label(s.label)))));
        }
        clear.push(dir);
        at = next;
    }
    movement(clear)
}

fn movement(route: Vec<Dir>) -> Option<Command> {
    (!route.is_empty()).then(|| Command::Move(route.into_iter().take(MAX_STEPS).collect()))
}

impl Pilot {
    pub fn new(game: &Game, prepared: bool) -> Self {
        Self {
            home: game.level().town_at(game.player().pos).unwrap(),
            prepared,
            memory: BTreeMap::new(),
            retreating: false,
            refuge: None,
            quarry: None,
            goal: None,
            retreats: 0,
        }
    }

    pub fn choose(&mut self, game: &Game) -> Command {
        let p = game.player();
        let level = game.level();
        let memory = self.memory.entry(p.at).or_default();
        // Town directions and streets are available at creation. Dungeon geometry is learned
        // only through the engine's actual exploration memory, not Deep::spawns or unseen tiles.
        if p.at == LevelId::Surface {
            for y in self.home.origin.1..self.home.origin.1 + Town::HEIGHT {
                for x in self.home.origin.0..self.home.origin.0 + Town::WIDTH {
                    memory.tiles.insert((x, y), level.tile((x, y)));
                }
            }
        } else {
            for y in p.pos.1 - SIGHT..=p.pos.1 + SIGHT {
                for x in p.pos.0 - SIGHT..=p.pos.0 + SIGHT {
                    let pos = (x, y);
                    if game.remembers(pos) {
                        memory.tiles.entry(pos).or_insert_with(|| level.tile(pos));
                        if game
                            .pile_at(pos)
                            .is_some_and(|pile| pile.items.iter().any(|&item| useful(item, p, self.prepared)))
                            && game.sees(pos)
                        {
                            memory.loot.insert(pos);
                        }
                    }
                }
            }
        }
        *memory.visits.entry(p.pos).or_default() += 1;
        let seen = game.sighted();
        let blocked: BTreeSet<_> = seen.iter().map(|s| s.creature.pos).collect();
        let near = seen.iter().find(|s| distance(p.pos, s.creature.pos) <= 1);
        let skittish = |s: &&isle_core::Sighted<'_>| {
            s.creature.phenotype().is_some_and(|ph| ph.temperament == blah_core::Temperament::Skittish)
        };
        // A fleeing bystander must not prevent looting while also being ignored as a target.
        let threatened = !level.safe(p.pos)
            && (near.is_some()
                || seen.iter().any(|s| s.creature.hunting || (!skittish(&s) && distance(p.pos, s.creature.pos) <= 3)));
        let potion = p.sack.contains(&Item::Potion);
        if p.hp * 3 < p.max_hp && threatened && potion {
            return Command::Quaff;
        }
        if p.at == LevelId::Surface {
            self.retreating = false;
            self.refuge = None;
            self.quarry = None;
            if p.hp < p.max_hp || p.focus < p.max_focus {
                return Command::Rest;
            }
        }
        // Both policies take shelter seriously. Returning to a landing costs real movement;
        // rest there uses the same free, safe recovery available to a player.
        if matches!(p.at, LevelId::Deep(..)) {
            if !self.retreating && p.hp * 2 < p.max_hp {
                self.retreating = true;
                self.retreats += 1;
                // Pick a refuge once. Re-selecting the nearest landing as foes enter and
                // leave sight can otherwise flip the route between opposite directions.
                self.refuge = path(memory, p.pos, &BTreeSet::new(), |at| level.safe(at)).map(|route| {
                    route.into_iter().fold(p.pos, |(x, y), dir| {
                        let (dx, dy) = dir.delta();
                        (x + dx, y + dy)
                    })
                });
            }
            if level.safe(p.pos) {
                if p.hp < p.max_hp || (self.prepared && p.focus < p.max_focus) {
                    return Command::Rest;
                }
                self.retreating = false;
                self.refuge = None;
            }
            if self.retreating && p.hp == p.max_hp {
                self.retreating = false;
                self.refuge = None;
            }
            if self.retreating {
                if let Some(refuge) = self.refuge
                    && let Some(command) = path(memory, p.pos, &blocked, |at| at == refuge).and_then(movement)
                {
                    return command;
                }
                // A sleeping foe across the only way back must not turn the retreat into a loop.
                if near.is_none() && !seen.iter().any(|s| s.creature.hunting) {
                    return Command::Rest;
                }
            }
        }
        // Skittish life flees and teaches nothing; chasing it in and out of sight only loops.
        // An unreachable foe (behind another in a corridor) would pull the pilot back each time it is seen.
        let reachable = |s: &&isle_core::Sighted<'_>| {
            path(memory, p.pos, &blocked, |at| distance(at, s.creature.pos) <= 1).is_some()
        };
        let target = near
            .or_else(|| seen.iter().find(|s| s.creature.hunting && reachable(s)))
            .or_else(|| seen.iter().find(|s| !skittish(s) && reachable(s)));
        if let Some(foe) = target {
            self.quarry = Some((foe.creature.id, foe.creature.pos));
        } else if self.quarry.is_some_and(|(_, at)| at == p.pos || distance(at, p.pos) <= 1) {
            self.quarry = None;
        }
        if threatened {
            if let Some(spell) = p.chanted {
                return Command::Cast(if spell.aimed() { target.map(|s| Target::Label(s.label)) } else { None });
            }
            if self.prepared && p.shield <= 2 && p.focus >= Spell::Shield.cost() {
                return Command::Chant(Spell::Shield);
            }
            if self.prepared
                && p.hp * 2 < p.max_hp
                && p.focus >= Spell::Heal.cost() + if p.armor.is_some_and(Armor::metal) { 2 } else { 0 }
            {
                return Command::Chant(Spell::Heal);
            }
            if let Some(foe) = near {
                return Command::Attack(Some(Target::Label(foe.label)));
            }
        }
        if !threatened {
            if let Some(&item) =
                p.sack.iter().find(|&&item| useful(item, p, self.prepared) && !matches!(item, Item::Potion))
            {
                return match item {
                    Item::Weapon(_) => Command::Wield(item_query(item)),
                    Item::Armor(_) | Item::Ring | Item::Relic(_) => Command::Wear(item_query(item)),
                    _ => unreachable!(),
                };
            }
            if let Some(pile) = game.pile_at(p.pos)
                && let Some(&item) = pile.items.iter().find(|&&item| useful(item, p, self.prepared))
            {
                if matches!(item, Item::Gold(_)) || p.sack.len() < isle_core::SACK {
                    return Command::Get(item_query(item));
                }
                if let Some(&junk) =
                    p.sack.iter().find(|&&i| !useful(i, p, self.prepared) && i != Item::Potion && !i.mythic())
                {
                    return Command::Drop(item_query(junk));
                }
            }
            memory.loot.remove(&p.pos);
        }
        if p.at == LevelId::Surface {
            let desired = if p.weapon != Weapon::LongSword && p.gold >= 80 {
                Some(Item::Weapon(Weapon::LongSword))
            } else if armor_value(p.armor) < Armor::Plate.protection() && p.gold >= 300 {
                Some(Item::Armor(Armor::Plate))
            } else if armor_value(p.armor) < Armor::Chainmail.protection() && p.gold >= 90 {
                Some(Item::Armor(Armor::Chainmail))
            } else {
                None
            };
            let goal = if desired.is_some() { self.home.shop() } else { self.home.stairs() };
            if p.pos == goal {
                return desired.map_or(Command::Down, |item| Command::Buy(item_query(item)));
            }
            return movement(path(memory, p.pos, &blocked, |at| at == goal).expect("town streets connect")).unwrap();
        }
        if let Some(foe) = target {
            if distance(p.pos, foe.creature.pos) <= 1 {
                return Command::Attack(Some(Target::Label(foe.label)));
            }
            if let Some(route) = path(memory, p.pos, &blocked, |at| distance(at, foe.creature.pos) <= 1)
                && let Some(command) = movement(route)
            {
                return command;
            }
        }
        // Keep pursuing the last foe toward where it was last seen instead of re-planning on sight alone.
        if target.is_none()
            && let Some((_, at)) = self.quarry
        {
            match path(memory, p.pos, &blocked, |cell| cell == at).and_then(movement) {
                Some(command) => return command,
                None => self.quarry = None,
            }
        }
        if let Some(command) = approach(memory, p.pos, &seen, |at| memory.loot.contains(&at)) {
            return command;
        }
        if game.conquered(p.at) {
            if level.tile(p.pos) == Tile::StairsDown {
                return Command::Down;
            }
            if let Some(command) = approach(memory, p.pos, &seen, |at| memory.tiles.get(&at) == Some(&Tile::StairsDown))
            {
                return command;
            }
        }
        // Commit to one exploration goal: re-choosing as creatures enter and leave sight flips direction.
        if let Some((at, goal)) = self.goal
            && at == p.at
            && goal != p.pos
            && let Some(command) = approach(memory, p.pos, &seen, |cell| cell == goal)
        {
            return command;
        }
        self.goal = None;
        let frontier = |at: Pos| {
            DIRS.iter().any(|dir| {
                let (dx, dy) = dir.delta();
                !memory.tiles.contains_key(&(at.0 + dx, at.1 + dy))
            })
        };
        // Do not stand still on a frontier: stepping onto it reveals its unseen neighbors.
        let destination = |route: Vec<Dir>| {
            route.into_iter().fold(p.pos, |(x, y), dir| {
                let (dx, dy) = dir.delta();
                (x + dx, y + dy)
            })
        };
        let goal =
            path(memory, p.pos, &BTreeSet::new(), |at| at != p.pos && frontier(at)).map(destination).or_else(|| {
                // If every corridor is known but a keeper has wandered, sweep the least visited cells.
                memory
                    .tiles
                    .iter()
                    .filter(|(at, tile)| tile.walkable() && **at != p.pos)
                    .min_by_key(|(at, _)| (memory.visits.get(at).copied().unwrap_or(0), distance(p.pos, **at), **at))
                    .map(|(&at, _)| at)
            });
        if let Some(goal) = goal
            && let Some(command) = approach(memory, p.pos, &seen, |cell| cell == goal)
        {
            self.goal = Some((p.at, goal));
            return command;
        }
        Command::Wait
    }
}
