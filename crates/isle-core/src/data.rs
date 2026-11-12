//! The fixed tables: creatures, gear, and spells.

use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// A hand weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weapon {
    /// Quick and light.
    Dagger,
    /// The common blade.
    ShortSword,
    /// A long and heavy blade.
    LongSword,
    /// Heavy and blunt.
    Mace,
    /// A walking stick.
    Staff,
}

impl Weapon {
    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Weapon::Dagger => "dagger",
            Weapon::ShortSword => "short sword",
            Weapon::LongSword => "long sword",
            Weapon::Mace => "mace",
            Weapon::Staff => "staff",
        }
    }

    /// Damage dealt, low and high.
    pub const fn damage(self) -> (i32, i32) {
        match self {
            Weapon::Dagger => (1, 4),
            Weapon::ShortSword => (2, 7),
            Weapon::LongSword => (3, 10),
            Weapon::Mace => (2, 8),
            Weapon::Staff => (1, 6),
        }
    }

    /// Bonus to hit.
    pub const fn accuracy(self) -> i32 {
        match self {
            Weapon::Dagger => 2,
            Weapon::ShortSword => 1,
            Weapon::LongSword | Weapon::Staff => 0,
            Weapon::Mace => -1,
        }
    }
}

/// A bow, fired one nocked arrow at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bow {
    /// Light, short range.
    Shortbow,
    /// Hits harder and farther.
    Longbow,
}

impl Bow {
    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Bow::Shortbow => "shortbow",
            Bow::Longbow => "longbow",
        }
    }

    /// Damage dealt, low and high.
    pub const fn damage(self) -> (i32, i32) {
        match self {
            Bow::Shortbow => (1, 6),
            Bow::Longbow => (2, 9),
        }
    }

    /// Cells an arrow flies.
    pub const fn range(self) -> i32 {
        match self {
            Bow::Shortbow => 6,
            Bow::Longbow => 8,
        }
    }
}

/// Body armor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Armor {
    /// Light hide.
    Leather,
    /// Linked rings.
    Chainmail,
    /// Heavy and slow to buy.
    Plate,
}

impl Armor {
    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Armor::Leather => "leather armor",
            Armor::Chainmail => "chainmail",
            Armor::Plate => "plate armor",
        }
    }

    /// Whether it is metal, which raises the Focus cost of most spells.
    pub const fn metal(self) -> bool {
        !matches!(self, Armor::Leather)
    }

    /// Armor class it adds.
    pub const fn protection(self) -> i32 {
        match self {
            Armor::Leather => 2,
            Armor::Chainmail => 4,
            Armor::Plate => 6,
        }
    }
}

/// A thing that can lie on the ground or in the sack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    /// A relic with a benefit and a cost while worn.
    Relic(crate::Relic),
    /// A gathered field-recovery herb.
    Herb(crate::Herb),
    /// A hand weapon.
    Weapon(Weapon),
    /// A bow.
    Bow(Bow),
    /// Body armor.
    Armor(Armor),
    /// A healing potion.
    Potion,
    /// The ring of warding.
    Ring,
    /// Some arrows.
    Arrows(u32),
    /// Some coins.
    Gold(u32),
}

impl Item {
    /// The item as the log names it, with a count where it has one.
    pub fn name(self) -> String {
        match self {
            Item::Relic(relic) => relic.name().into(),
            Item::Herb(herb) => herb.name().into(),
            Item::Weapon(w) => w.name().into(),
            Item::Bow(b) => b.name().into(),
            Item::Armor(a) => a.name().into(),
            Item::Potion => "healing potion".into(),
            Item::Ring => "ring of warding".into(),
            Item::Arrows(1) => "an arrow".into(),
            Item::Arrows(n) => format!("{n} arrows"),
            Item::Gold(n) => format!("{n} gold"),
        }
    }

    /// The words a player may type for it.
    pub(crate) fn words(self) -> &'static str {
        match self {
            Item::Relic(relic) => relic.words(),
            Item::Herb(crate::Herb::Woundleaf) => "woundleaf herb",
            Item::Herb(crate::Herb::Bogmint) => "bogmint mint herb",
            Item::Herb(crate::Herb::Sunroot) => "sunroot root herb",
            Item::Weapon(w) => w.name(),
            Item::Bow(b) => match b {
                Bow::Shortbow => "shortbow short bow",
                Bow::Longbow => "longbow long bow",
            },
            Item::Armor(a) => match a {
                Armor::Leather => "leather armor",
                Armor::Chainmail => "chainmail chain mail armor",
                Armor::Plate => "plate armor",
            },
            Item::Potion => "healing potion",
            Item::Ring => "ring of warding",
            Item::Arrows(_) => "arrows arrow",
            Item::Gold(_) => "gold coins",
        }
    }

    /// Whether `query` names this item: every typed word starts some word of its name.
    pub(crate) fn matches(self, query: &str) -> bool {
        let words: Vec<&str> = self.words().split(' ').collect();
        let mut typed = query.split_whitespace().peekable();
        typed.peek().is_some() && typed.all(|t| words.iter().any(|w| w.starts_with(t)))
    }
}

/// The outfitter's wares and prices. Arrows come ten to a bundle.
pub const SHOP: [(Item, u32); 11] = [
    (Item::Arrows(10), 10),
    (Item::Potion, 25),
    (Item::Weapon(Weapon::Dagger), 10),
    (Item::Weapon(Weapon::ShortSword), 30),
    (Item::Weapon(Weapon::Mace), 40),
    (Item::Weapon(Weapon::LongSword), 80),
    (Item::Bow(Bow::Shortbow), 40),
    (Item::Bow(Bow::Longbow), 120),
    (Item::Armor(Armor::Leather), 20),
    (Item::Armor(Armor::Chainmail), 90),
    (Item::Armor(Armor::Plate), 300),
];

/// What the outfitter pays for `item`: half its price, or a token sum for things he doesn't stock.
pub fn sale_price(item: Item) -> u32 {
    match item {
        Item::Relic(_) => 75,
        Item::Herb(_) => 2,
        Item::Ring => 150,
        Item::Gold(_) | Item::Arrows(_) => 0,
        _ => SHOP.iter().find(|(i, _)| *i == item).map_or(5, |&(_, price)| price / 2),
    }
}

/// What a treasure is reckoned at, which is what the outfitter charges for it. The ring is not sold.
fn worth(item: Item) -> u32 {
    match item {
        Item::Ring => 300,
        _ => SHOP.iter().find(|(i, _)| *i == item).map_or(0, |&(_, price)| price),
    }
}

/// A find worth about `value` gold: coins, and maybe the best kind of thing that value buys. The bigger the
/// value, the better the gear, since nothing priced at under a sixth of it is worth carrying off.
pub(crate) fn treasure(value: u32, item_chance: i32, rng: &mut Rng) -> Vec<Item> {
    let mut found = Vec::new();
    if rng.chance(item_chance) {
        let floor = value.min(1800) / 6;
        let goods: Vec<Item> = SHOP
            .iter()
            .map(|&(item, _)| item)
            .chain([Item::Ring])
            .filter(|&item| (floor..=value).contains(&worth(item)))
            .collect();
        if !goods.is_empty() {
            found.push(goods[rng.below(goods.len() as u32) as usize]);
        }
        if rng.chance(25) && value >= 25 {
            found.push(Item::Potion);
        }
    }
    let coins = rng.range((value / 4) as i32, (value / 2) as i32);
    if coins > 0 {
        found.push(Item::Gold(coins as u32));
    }
    found
}
/// A spell, chanted one round and cast the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spell {
    /// A crackling bolt at one target in sight.
    Spark,
    /// A ward that turns arrows and blows aside for a while.
    Shield,
    /// Closes the caster's own wounds.
    Heal,
    /// Freezes one target in sight for a few rounds.
    Stun,
}

impl Spell {
    /// Every spell.
    pub const ALL: [Spell; 4] = [Spell::Spark, Spell::Shield, Spell::Heal, Spell::Stun];

    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Spell::Spark => "spark",
            Spell::Shield => "shield",
            Spell::Heal => "heal",
            Spell::Stun => "stun",
        }
    }

    /// Whether it needs a target.
    pub const fn aimed(self) -> bool {
        matches!(self, Spell::Spark | Spell::Stun)
    }

    /// Base Focus spent when cast, before a metal-armor penalty.
    pub const fn cost(self) -> i32 {
        match self {
            Spell::Spark => 3,
            Spell::Shield | Spell::Heal => 4,
            Spell::Stun => 5,
        }
    }
}

/// A kind of creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A big rat.
    Rat,
    /// A woods wolf.
    Wolf,
    /// A goblin with a club.
    Goblin,
    /// A goblin with a bow.
    GoblinArcher,
    /// An orc.
    Orc,
    /// Old bones, hard to hurt with arrows.
    Skeleton,
    /// A pit troll, kept as a fixed-table fixture.
    Troll,
    /// Cinder Watch's armored, one-time keeper.
    CinderWarden,
    /// Cinder Watch's one-time bow guard.
    AshSentry,
    /// An individual drawn from a region's evolved life.
    Wild,
}

/// The numbers behind a kind of creature.
#[derive(Clone, Debug)]
pub struct Stats {
    /// What the log calls one.
    pub name: String,
    /// Starting and full health.
    pub hp: i32,
    /// What an attack roll must reach to hit it.
    pub ac: i32,
    /// Bonus to its own attack rolls.
    pub hit: i32,
    /// Melee damage, low and high.
    pub damage: (i32, i32),
    /// Range and damage of its shots, if it shoots.
    pub shot: Option<(i32, (i32, i32))>,
    /// Experience for killing it.
    pub xp: u32,
    /// Coins it carries, low and high.
    pub gold: (i32, i32),
    /// Health it regains each round.
    pub regen: i32,
    /// Whether arrows do half damage.
    pub arrow_proof: bool,
}

impl Kind {
    /// Its numbers.
    pub fn stats(self) -> Stats {
        let base = Stats {
            name: String::new(),
            hp: 1,
            ac: 10,
            hit: 0,
            damage: (1, 1),
            shot: None,
            xp: 0,
            gold: (0, 0),
            regen: 0,
            arrow_proof: false,
        };
        match self {
            Kind::Rat => {
                Stats { name: "rat".into(), hp: 5, ac: 9, hit: 1, damage: (1, 3), xp: 5, gold: (0, 2), ..base }
            }
            Kind::Wolf => Stats { name: "wolf".into(), hp: 10, ac: 12, hit: 3, damage: (2, 5), xp: 15, ..base },
            Kind::Goblin => {
                Stats { name: "goblin".into(), hp: 9, ac: 11, hit: 2, damage: (1, 6), xp: 15, gold: (1, 8), ..base }
            }
            Kind::GoblinArcher => Stats {
                name: "goblin archer".into(),
                hp: 8,
                ac: 11,
                hit: 3,
                damage: (1, 3),
                shot: Some((5, (1, 6))),
                xp: 22,
                gold: (2, 10),
                ..base
            },
            Kind::Orc => {
                Stats { name: "orc".into(), hp: 16, ac: 13, hit: 4, damage: (2, 8), xp: 35, gold: (4, 15), ..base }
            }
            Kind::Skeleton => Stats {
                name: "skeleton".into(),
                hp: 18,
                ac: 12,
                hit: 4,
                damage: (2, 7),
                xp: 40,
                arrow_proof: true,
                ..base
            },
            Kind::Troll => Stats {
                name: "pit troll".into(),
                hp: 70,
                ac: 14,
                hit: 6,
                damage: (4, 12),
                xp: 400,
                gold: (80, 120),
                regen: 2,
                ..base
            },
            Kind::CinderWarden => Stats {
                name: "cinder warden".into(),
                hp: 20,
                ac: 12,
                hit: 3,
                damage: (2, 5),
                xp: 55,
                arrow_proof: true,
                ..base
            },
            Kind::AshSentry => Stats {
                name: "ash sentry".into(),
                hp: 9,
                ac: 11,
                hit: 2,
                damage: (1, 3),
                shot: Some((5, (1, 3))),
                xp: 25,
                ..base
            },
            Kind::Wild => Stats { name: "creature".into(), ..base },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_words_find_items_by_any_word_prefix() {
        assert!(Item::Bow(Bow::Shortbow).matches("bow"));
        assert!(Item::Bow(Bow::Longbow).matches("long bow"));
        assert!(Item::Weapon(Weapon::LongSword).matches("sw"));
        assert!(Item::Arrows(3).matches("arrow"));
        assert!(Item::Potion.matches("pot"));
        assert!(!Item::Potion.matches("sword"));
        assert!(!Item::Potion.matches(""));
    }

    #[test]
    fn the_shop_buys_back_at_half() {
        assert_eq!(sale_price(Item::Bow(Bow::Longbow)), 60);
        assert_eq!(sale_price(Item::Weapon(Weapon::Staff)), 5);
        assert_eq!(sale_price(Item::Ring), 150);
    }

    #[test]
    fn bigger_treasures_hold_better_gear_and_more_gold() {
        let mut rng = Rng::new(9);
        let mean_worth = |value: u32, rng: &mut Rng| {
            let finds: Vec<Vec<Item>> = (0..400).map(|_| treasure(value, 100, rng)).collect();
            let gear: Vec<u32> =
                finds.iter().flatten().filter(|i| !matches!(i, Item::Gold(_))).map(|&i| worth(i)).collect();
            let gold: u32 = finds.iter().flatten().map(|i| if let Item::Gold(n) = i { *n } else { 0 }).sum();
            (gear.iter().sum::<u32>() / gear.len().max(1) as u32, gold / 400)
        };
        let (small, large) = (mean_worth(30, &mut rng), mean_worth(400, &mut rng));
        assert!(large.0 > small.0 * 4 && large.1 > small.1 * 8, "{small:?} {large:?}");
        assert!((0..400).any(|_| treasure(600, 100, &mut rng).contains(&Item::Armor(Armor::Plate))));
        assert!((0..400).all(|_| !treasure(30, 100, &mut rng).contains(&Item::Ring)));
        assert!(treasure(0, 0, &mut rng).is_empty());
    }
}
