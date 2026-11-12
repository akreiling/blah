//! The player's figure, drawn in code from what they practice and what they carry.
//!
//! Isle has no classes, so the figure follows the character: the highest weapon, magic, or stealth practice
//! sets its calling (hood, hat, robe, cape), and the equipped weapon, bow, armor, and ring are drawn on top.
//! Vitality and Focus are capacities every character trains by surviving, so they never pick the calling.

use blah_core::Rank;
use image::RgbaImage;
use isle_core::{Armor, Bow, Player, Practice, Weapon};

const SIDE: usize = 32;

/// The look a run of practice grows into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Calling {
    /// Nothing practiced beyond novice yet.
    Wanderer,
    /// Blades or blunt weapons: a cape, and a helm under metal armor.
    Warrior,
    /// Daggers or stealth: a hood and a dark cloak.
    Rogue,
    /// Archery: a feathered cap.
    Ranger,
    /// Staff, evocation, or binding: a pointed hat and robe in the art's color.
    Mage,
    /// Restoration or warding: a pale robe, circlet, and holy mark.
    Cleric,
}

/// Everything that changes the figure. Equal looks share one cached image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Look {
    pub calling: Calling,
    /// The calling's cloth color; magic callings take their discipline's color.
    pub accent: [u8; 3],
    /// Skilled or better in the calling's practice: gold trim.
    pub trim: bool,
    pub weapon: Weapon,
    pub bow: Option<Bow>,
    pub armor: Option<Armor>,
    pub ring: bool,
}

impl Look {
    pub fn of(player: &Player) -> Look {
        let held = Practice::weapon(player.weapon);
        let (_, best) = Practice::ALL
            .into_iter()
            .enumerate()
            .filter(|(_, p)| !matches!(p, Practice::Vitality | Practice::Focus))
            // Ties go to the weapon in hand, then to the earlier practice in the skills list.
            .max_by_key(|&(i, p)| (player.practice.get(p).rank(), p == held, std::cmp::Reverse(i)))
            .expect("practices are listed");
        let rank = player.practice.get(best).rank();
        let (calling, accent) = if rank == Rank::Novice {
            (Calling::Wanderer, [140, 104, 66])
        } else {
            match best {
                Practice::Dagger | Practice::Stealth => (Calling::Rogue, [56, 64, 62]),
                Practice::Blades | Practice::Blunt => (Calling::Warrior, [164, 48, 44]),
                Practice::Archery => (Calling::Ranger, [66, 110, 54]),
                Practice::Evocation => (Calling::Mage, [204, 90, 36]),
                Practice::Binding => (Calling::Mage, [124, 76, 178]),
                Practice::Staff => (Calling::Mage, [58, 96, 186]),
                Practice::Warding => (Calling::Cleric, [150, 190, 226]),
                _ => (Calling::Cleric, [234, 230, 214]),
            }
        };
        Look {
            calling,
            accent,
            trim: rank >= Rank::Skilled,
            weapon: player.weapon,
            bow: player.bow,
            armor: player.armor,
            ring: player.ring,
        }
    }
}

const OUTLINE: [u8; 3] = [24, 20, 28];
const MARKER: [u8; 3] = [255, 242, 122];
const SKIN: [u8; 3] = [232, 188, 148];
const HAIR: [u8; 3] = [92, 58, 36];
const EYES: [u8; 3] = [30, 24, 30];
const PANTS: [u8; 3] = [84, 70, 58];
const BOOTS: [u8; 3] = [66, 44, 30];
const BELT: [u8; 3] = [70, 44, 26];
const GOLD: [u8; 3] = [226, 184, 72];
const WOOD: [u8; 3] = [140, 92, 48];
const STRING: [u8; 3] = [222, 216, 198];
const STEEL: [u8; 3] = [212, 218, 228];
const STEEL_DARK: [u8; 3] = [118, 124, 136];
const LEATHER: [u8; 3] = [122, 80, 46];
const CHAIN: [[u8; 3]; 2] = [[152, 158, 168], [108, 114, 126]];
const PLATE: [[u8; 3]; 3] = [[198, 206, 216], [240, 243, 248], [128, 136, 150]];
const ASHEN: [[u8; 3]; 3] = [[38, 35, 40], [92, 85, 87], [61, 56, 62]];
const EMBER: [u8; 3] = [255, 98, 34];

fn darker(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (u16::from(v) * 70 / 100) as u8)
}

struct Canvas([[Option<[u8; 3]>; SIDE]; SIDE]);

impl Canvas {
    fn dot(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if (0..SIDE as i32).contains(&x) && (0..SIDE as i32).contains(&y) {
            self.0[y as usize][x as usize] = Some(c);
        }
    }

    /// Inclusive on both corners.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 3]) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.dot(x, y, c);
            }
        }
    }
}

/// The figure as a 32×32 transparent image, outlined like the object sheet, standing on a marker ring so the
/// player is easy to find among creatures.
pub fn draw(look: Look) -> RgbaImage {
    let mut c = Canvas([[None; SIDE]; SIDE]);
    let robed = matches!(look.calling, Calling::Mage | Calling::Cleric);
    let cloth = look.accent;

    // Behind the body: the bow and any cape.
    if let Some(bow) = look.bow {
        let (top, bottom): (i32, i32) = if bow == Bow::Longbow { (4, 25) } else { (8, 22) };
        let mid = (top + bottom) / 2;
        for y in top..=bottom {
            let bend = 3 * (y - mid).abs() / (mid - top);
            c.dot(7 + bend, y, WOOD);
        }
        c.rect(10, top, 10, bottom, STRING);
    }
    match look.calling {
        Calling::Warrior => {
            c.rect(10, 12, 21, 24, darker(cloth));
            if look.trim {
                c.rect(10, 24, 21, 24, GOLD);
            }
        }
        Calling::Rogue => {
            c.rect(9, 11, 22, 25, cloth);
            if look.trim {
                c.rect(9, 25, 22, 25, GOLD);
            }
        }
        _ => {}
    }

    // Legs, or a robe that flares toward the hem.
    if robed {
        for y in 20..=26 {
            let flare = (y - 20) / 3;
            c.rect(11 - flare, y, 20 + flare, y, cloth);
        }
        c.rect(12, 21, 12, 25, darker(cloth));
        if look.trim {
            c.rect(9, 26, 22, 26, GOLD);
        }
        c.rect(12, 27, 14, 27, BOOTS);
        c.rect(17, 27, 19, 27, BOOTS);
    } else {
        c.rect(12, 21, 14, 24, PANTS);
        c.rect(17, 21, 19, 24, PANTS);
        c.rect(12, 25, 14, 26, BOOTS);
        c.rect(17, 25, 19, 26, BOOTS);
    }

    // Torso and sleeves: armor if worn, otherwise the calling's cloth.
    let body = match look.armor {
        None => cloth,
        Some(Armor::Leather) => LEATHER,
        Some(Armor::Chainmail) => CHAIN[0],
        Some(Armor::Plate) => PLATE[0],
        Some(Armor::AshenCarapace) => ASHEN[0],
    };
    c.rect(11, 13, 20, 20, body);
    c.rect(9, 14, 10, 19, body);
    c.rect(21, 14, 22, 19, body);
    match look.armor {
        Some(Armor::Leather) => {
            for i in 0..6 {
                c.dot(12 + i, 14 + i, BELT);
            }
        }
        Some(Armor::Chainmail) => {
            for y in 13..=20 {
                for x in 9..=22 {
                    if (x + y) % 2 == 0 && c.0[y as usize][x as usize] == Some(CHAIN[0]) {
                        c.dot(x, y, CHAIN[1]);
                    }
                }
            }
        }
        Some(Armor::Plate) => {
            c.rect(12, 14, 12, 19, PLATE[1]);
            c.rect(20, 14, 20, 20, PLATE[2]);
            c.rect(9, 14, 10, 14, PLATE[1]);
            c.rect(21, 14, 22, 14, PLATE[1]);
        }
        Some(Armor::AshenCarapace) => {
            c.rect(9, 13, 12, 14, ASHEN[1]);
            c.rect(19, 13, 22, 14, ASHEN[1]);
            c.rect(10, 15, 11, 17, ASHEN[2]);
            c.rect(20, 15, 21, 17, ASHEN[2]);
        }
        None => {}
    }
    if look.calling == Calling::Warrior && look.armor.is_some() {
        c.rect(14, 14, 17, 20, cloth);
    }
    if look.calling == Calling::Cleric {
        c.rect(15, 14, 16, 18, GOLD);
        c.rect(14, 15, 17, 15, GOLD);
    }
    if look.armor == Some(Armor::AshenCarapace) {
        c.rect(14, 15, 14, 15, EMBER);
        c.rect(15, 16, 15, 17, EMBER);
        c.rect(14, 18, 14, 18, EMBER);
        c.rect(15, 19, 17, 19, EMBER);
        c.dot(18, 16, [255, 176, 72]);
    }
    c.rect(11, 20, 20, 20, BELT);
    c.rect(15, 20, 16, 20, GOLD);
    c.rect(9, 20, 10, 21, SKIN);
    c.rect(21, 20, 22, 21, SKIN);
    if look.ring {
        c.dot(9, 21, GOLD);
    }

    // Head and headgear.
    c.rect(15, 12, 16, 12, SKIN);
    c.rect(12, 6, 19, 11, SKIN);
    c.rect(13, 5, 18, 5, SKIN);
    c.dot(14, 9, EYES);
    c.dot(17, 9, EYES);
    let helmed = look.calling == Calling::Warrior
        && matches!(look.armor, Some(Armor::Chainmail | Armor::Plate | Armor::AshenCarapace));
    match look.calling {
        Calling::Rogue => {
            c.rect(11, 3, 20, 6, cloth);
            c.rect(11, 7, 12, 12, cloth);
            c.rect(19, 7, 20, 12, cloth);
            c.rect(13, 7, 18, 7, darker(SKIN));
        }
        Calling::Ranger => {
            c.rect(12, 4, 19, 6, HAIR);
            c.rect(12, 2, 19, 5, cloth);
            c.rect(11, 5, 20, 5, darker(cloth));
            c.rect(20, 0, 20, 3, [196, 60, 48]);
            c.rect(21, 1, 21, 2, [196, 60, 48]);
            if look.trim {
                c.rect(12, 4, 19, 4, GOLD);
            }
        }
        Calling::Mage => {
            c.rect(12, 6, 12, 8, HAIR);
            c.rect(19, 6, 19, 8, HAIR);
            c.rect(10, 5, 21, 5, darker(cloth));
            for (y, (x0, x1)) in [(4, (12, 19)), (3, (13, 18)), (2, (14, 17)), (1, (15, 16)), (0, (16, 17))] {
                c.rect(x0, y, x1, y, cloth);
            }
            if look.trim {
                c.rect(12, 4, 19, 4, GOLD);
            }
        }
        Calling::Cleric => {
            c.rect(12, 4, 19, 6, HAIR);
            c.rect(12, 6, 19, 6, GOLD);
            c.rect(15, 6, 16, 6, cloth);
        }
        Calling::Warrior if helmed => {
            let (steel, light) = match look.armor {
                Some(Armor::Plate) => (PLATE[0], PLATE[1]),
                Some(Armor::AshenCarapace) => (ASHEN[0], ASHEN[1]),
                _ => (CHAIN[0], STEEL),
            };
            c.rect(12, 3, 19, 7, steel);
            c.rect(13, 3, 18, 3, light);
            c.rect(15, 8, 16, 9, steel);
            if look.armor == Some(Armor::AshenCarapace) {
                c.rect(14, 7, 17, 7, EMBER);
            }
        }
        _ => {
            c.rect(12, 4, 19, 6, HAIR);
            c.rect(12, 7, 12, 8, HAIR);
            c.rect(19, 7, 19, 8, HAIR);
        }
    }

    // The hand weapon, held upright at the figure's right.
    match look.weapon {
        Weapon::Dagger => {
            c.rect(23, 15, 23, 18, STEEL);
            c.rect(22, 19, 24, 19, STEEL_DARK);
            c.rect(23, 20, 23, 21, WOOD);
        }
        Weapon::ShortSword | Weapon::LongSword => {
            let tip = if look.weapon == Weapon::LongSword { 3 } else { 10 };
            c.rect(23, tip, 24, 18, STEEL);
            c.rect(24, tip + 1, 24, 18, STEEL_DARK);
            c.rect(21, 19, 26, 19, GOLD);
            c.rect(23, 20, 24, 22, WOOD);
        }
        Weapon::Oathfang => {
            for (x, y) in [
                (24, 4),
                (24, 5),
                (25, 6),
                (25, 7),
                (25, 8),
                (24, 9),
                (24, 10),
                (23, 11),
                (22, 12),
                (22, 13),
                (21, 14),
                (20, 15),
                (20, 16),
            ] {
                c.rect(x - 1, y, x, y, [35, 30, 36]);
            }
            for (x, y) in [(24, 5), (24, 7), (24, 8), (23, 10), (22, 12), (21, 13), (20, 15)] {
                c.dot(x, y, EMBER);
            }
            c.rect(19, 16, 24, 16, GOLD);
            c.rect(22, 17, 23, 19, WOOD);
        }
        Weapon::Mace => {
            c.rect(23, 12, 23, 22, WOOD);
            c.rect(22, 8, 24, 11, STEEL_DARK);
            c.dot(21, 9, STEEL);
            c.dot(25, 9, STEEL);
            c.dot(23, 7, STEEL);
        }
        Weapon::Staff => {
            c.rect(23, 4, 23, 27, WOOD);
            let knob = if look.calling == Calling::Mage { cloth } else { darker(WOOD) };
            c.rect(22, 1, 24, 3, knob);
        }
    }

    let mut out = RgbaImage::new(SIDE as u32, SIDE as u32);
    let filled = |x: i32, y: i32| {
        (0..SIDE as i32).contains(&x) && (0..SIDE as i32).contains(&y) && c.0[y as usize][x as usize].is_some()
    };
    for y in 0..SIDE as i32 {
        for x in 0..SIDE as i32 {
            let beside = |test: &dyn Fn(i32, i32) -> bool| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| test(x + dx, y + dy))
            };
            let pixel = match c.0[y as usize][x as usize] {
                Some(color) => Some((color, 255)),
                None if beside(&filled) => Some((OUTLINE, 255)),
                None if marker(x, y) => Some((MARKER, 255)),
                // A translucent shadow inside the ring and a rim around it ground the figure and keep the
                // ring legible on pale sand and stone.
                None if shadow(x, y) || beside(&marker) => Some((OUTLINE, SHADOW)),
                None => None,
            };
            if let Some(([r, g, b], a)) = pixel {
                out.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
            }
        }
    }
    out
}

const SHADOW: u8 = 110;

fn shadow(x: i32, y: i32) -> bool {
    let (dx, dy) = ((x as f32 - 15.5) / 8.5, (y as f32 - 29.0) / 2.2);
    dx * dx + dy * dy <= 1.0
}

/// A ring at the figure's feet; the figure covers its far side.
fn marker(x: i32, y: i32) -> bool {
    match y {
        27 | 31 => (11..=20).contains(&x),
        28 | 30 => matches!(x, 8..=10 | 21..=23),
        29 => matches!(x, 7 | 24),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isle_core::Game;

    fn player(practices: &[(Practice, u32)], edit: impl FnOnce(&mut serde_json::Value)) -> Player {
        let mut game = Game::new(7);
        game.answer("Ash");
        let mut save = serde_json::to_value(&game).unwrap();
        for &(practice, points) in practices {
            let i = Practice::ALL.iter().position(|&p| p == practice).unwrap();
            save["player"]["practice"]["skills"][i]["practice"] = serde_json::json!(points);
        }
        edit(&mut save["player"]);
        serde_json::from_value::<Game>(save).unwrap().player().clone()
    }

    #[test]
    fn the_highest_practice_picks_the_calling_and_capacities_never_do() {
        let fresh = player(&[], |_| {});
        assert_eq!(Look::of(&fresh).calling, Calling::Wanderer);
        let cases = [
            (Practice::Blades, Calling::Warrior),
            (Practice::Blunt, Calling::Warrior),
            (Practice::Dagger, Calling::Rogue),
            (Practice::Stealth, Calling::Rogue),
            (Practice::Archery, Calling::Ranger),
            (Practice::Evocation, Calling::Mage),
            (Practice::Binding, Calling::Mage),
            (Practice::Staff, Calling::Mage),
            (Practice::Restoration, Calling::Cleric),
            (Practice::Warding, Calling::Cleric),
        ];
        for (practice, calling) in cases {
            let look = Look::of(&player(&[(practice, 1_000)], |_| {}));
            assert_eq!((look.calling, look.trim), (calling, false), "{practice:?}");
            assert!(Look::of(&player(&[(practice, 4_000)], |_| {})).trim, "skilled earns trim");
        }
        let survivor = player(&[(Practice::Vitality, 30_000), (Practice::Focus, 30_000)], |_| {});
        assert_eq!(Look::of(&survivor).calling, Calling::Wanderer);
        let higher = player(&[(Practice::Blades, 1_000), (Practice::Stealth, 4_000)], |_| {});
        assert_eq!(Look::of(&higher).calling, Calling::Rogue);
        // Equal ranks: the weapon in hand wins, whichever practice comes first in the list.
        let tie = |weapon: &str| {
            Look::of(&player(&[(Practice::Dagger, 1_000), (Practice::Staff, 1_000)], |p| {
                p["weapon"] = serde_json::json!(weapon);
            }))
            .calling
        };
        assert_eq!(tie("Staff"), Calling::Mage);
        assert_eq!(tie("ShortSword"), Calling::Rogue);
    }

    #[test]
    fn every_calling_and_piece_of_gear_changes_the_figure() {
        let base = Look::of(&player(&[], |p| {
            p["weapon"] = serde_json::json!("ShortSword");
            p["bow"] = serde_json::Value::Null;
            p["armor"] = serde_json::Value::Null;
            p["ring"] = serde_json::json!(false);
        }));
        let plain = draw(base);
        let mut variants = vec![plain.clone()];
        for practice in
            [Practice::Blades, Practice::Dagger, Practice::Archery, Practice::Evocation, Practice::Restoration]
        {
            let look = Look::of(&player(&[(practice, 1_000)], |_| {}));
            variants.push(draw(Look { weapon: base.weapon, bow: None, armor: None, ring: false, ..look }));
        }
        for weapon in [Weapon::Dagger, Weapon::LongSword, Weapon::Mace, Weapon::Staff, Weapon::Oathfang] {
            variants.push(draw(Look { weapon, ..base }));
        }
        for armor in [Armor::Leather, Armor::Chainmail, Armor::Plate, Armor::AshenCarapace] {
            variants.push(draw(Look { armor: Some(armor), ..base }));
        }
        for bow in [Bow::Shortbow, Bow::Longbow] {
            variants.push(draw(Look { bow: Some(bow), ..base }));
        }
        variants.push(draw(Look { ring: true, ..base }));
        variants.push(draw(Look { trim: true, calling: Calling::Mage, ..base }));
        for (i, a) in variants.iter().enumerate() {
            for b in &variants[i + 1..] {
                assert_ne!(a, b, "variant {i} must look different");
            }
            assert!(a.pixels().all(|p| [0, SHADOW, 255].contains(&p[3])), "opaque art over a single shadow alpha");
            assert!(a.pixels().all(|p| p[3] != SHADOW || p.0[..3] == OUTLINE), "only the shadow is translucent");
            assert_eq!(a.get_pixel(0, 0)[3], 0, "corners stay transparent");
            assert_eq!(a.get_pixel(29, 29)[3], 0);
            let marker = (0..32).filter(|&x| a.get_pixel(x, 31).0[..3] == MARKER).count();
            assert_eq!(marker, 10, "the marker ring is never covered at the front");
        }
        assert_eq!(draw(base), plain, "deterministic");
    }
}
