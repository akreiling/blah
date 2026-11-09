//! Placeholder names for regions and species: the offline fallback until a remote model renames them, or
//! forever.
//!
//! Every name is a pure function of the seed and what it names, drawn from the word lists below. All of
//! the wording lives in this file as data.
//!
//! A species name is a biome epithet and its clade's root: "Dune Skitter", "Fen Skitter". The root says
//! which lineage it is and the epithet where it lives, so the relationship shows in the name. Every
//! species of one region shares the region's epithet.
//!
//! Region names come in four shapes:
//!
//! - "the Glass Marsh": an adjective, either general or the biome's own, and a biome noun
//! - "Ashfold Hills": a compound proper name built from English-like parts
//! - "Kelamar Taiga": a proper name built from syllables
//! - "the Fens of Valdor": a biome noun and either kind of proper name
//!
//! General adjectives say nothing about heat or wetness, so they never contradict the biome they land on;
//! climate words live in each biome's own list.

use crate::biome::BiomeId;
use crate::clade::Family;
use crate::hash::{combine, derive, mix};
use crate::region::RegionId;

/// One biome's flavor: what to call a place of this kind, what places of this kind are like, and what to
/// call the things that live there.
struct Flavor {
    /// Must equal a name in [`BIOMES`](crate::BIOMES); `every_biome_has_a_flavor` checks.
    biome: &'static str,
    nouns: &'static [&'static str],
    adjectives: &'static [&'static str],
    /// First words of species names. Single words, so "Dune Skitter" stays two.
    epithets: &'static [&'static str],
}

const fn flavor(
    biome: &'static str,
    nouns: &'static [&'static str],
    adjectives: &'static [&'static str],
    epithets: &'static [&'static str],
) -> Flavor {
    Flavor { biome, nouns, adjectives, epithets }
}

#[rustfmt::skip]
const FLAVORS: &[Flavor] = &[
    flavor("Deep water",
        &["Deep", "Deeps", "Abyss", "Sea", "Gulf", "Trench", "Expanse"],
        &["Drowned", "Sunless", "Fathomless", "Restless", "Salt", "Wide"],
        &["Abyss", "Deep", "Brine", "Gulf", "Tide"]),
    flavor("Shallow water",
        &["Shallows", "Shoals", "Sound", "Bay", "Lagoon", "Strait", "Reach"],
        &["Clear", "Tidal", "Rippling", "Sandy", "Bright", "Wading"],
        &["Shoal", "Reef", "Lagoon", "Surf", "Kelp"]),
    flavor("Snow peaks",
        &["Peaks", "Spires", "Crowns", "Horns", "Teeth", "Summits"],
        &["White", "Frozen", "Frostbitten", "Howling", "Icebound", "Shining"],
        &["Peak", "Ice", "Crag", "Summit", "Glacier"]),
    flavor("Mountains",
        &["Mountains", "Crags", "Heights", "Range", "Tors", "Ridges"],
        &["Jagged", "Stern", "Thundering", "Granite", "High", "Shattered"],
        &["Crag", "Tor", "Scree", "Ridge", "Granite"]),
    flavor("Badlands",
        &["Badlands", "Breaks", "Scarps", "Mesas", "Wastes", "Gullies"],
        &["Scorched", "Red", "Cracked", "Rust", "Bleached", "Crumbling"],
        &["Mesa", "Rust", "Gully", "Scarp", "Cinder"]),
    flavor("Hills",
        &["Hills", "Downs", "Rise", "Knolls", "Fells", "Uplands"],
        &["Rolling", "Windswept", "Gentle", "Barrow", "Folded", "Tumbled"],
        &["Hill", "Down", "Fell", "Barrow", "Knoll"]),
    flavor("Tundra",
        &["Tundra", "Barrens", "Steppe", "Flats", "Wastes", "Expanse"],
        &["Frozen", "White", "Frostbitten", "Windswept", "Pale", "Bitter"],
        &["Frost", "Rime", "Snow", "Tundra", "Permafrost"]),
    flavor("Beach",
        &["Strand", "Shore", "Sands", "Coast", "Shingle", "Spit"],
        &["Bright", "Shell", "Driftwood", "Gull", "Tidewashed", "Pearl"],
        &["Shore", "Strand", "Shingle", "Dune", "Shell"]),
    flavor("Taiga",
        &["Taiga", "Pinewood", "Firs", "Pines", "Spruces", "Needlewood"],
        &["Snowbound", "Hushed", "Frosted", "Dark", "Weeping", "Resin"],
        &["Pine", "Spruce", "Needle", "Fir", "Resin"]),
    flavor("Swamp",
        &["Marsh", "Fen", "Fens", "Mire", "Bog", "Swamp", "Slough"],
        &["Sodden", "Sunken", "Murky", "Misty", "Reedy", "Weeping"],
        &["Fen", "Bog", "Mire", "Marsh", "Reed"]),
    flavor("Desert",
        &["Desert", "Sands", "Dunes", "Erg", "Waste", "Barrens"],
        &["Burning", "Thirsting", "Bleached", "Shimmering", "Scorched", "Endless"],
        &["Dune", "Sand", "Sun", "Dust", "Scorch"]),
    flavor("Savanna",
        &["Savanna", "Veld", "Plains", "Grasslands", "Scrub"],
        &["Golden", "Sunlit", "Dusty", "Thorn", "Tawny", "Baking"],
        &["Veld", "Thorn", "Acacia", "Dust", "Tawny"]),
    flavor("Jungle",
        &["Jungle", "Tangle", "Wilds", "Thicket", "Canopy", "Rainwood"],
        &["Green", "Tangled", "Steaming", "Humming", "Verdant", "Vine"],
        &["Vine", "Canopy", "Jungle", "Orchid", "Liana"]),
    flavor("Forest",
        &["Forest", "Wood", "Weald", "Holt", "Woods", "Greenwood"],
        &["Elder", "Mossy", "Deep", "Shaded", "Oaken", "Dim"],
        &["Wood", "Oak", "Moss", "Bracken", "Glade"]),
    flavor("Grassland",
        &["Plains", "Meadows", "Fields", "Prairie", "Leas", "Grasslands"],
        &["Open", "Wide", "Long", "Gentle", "Wildflower", "Swaying"],
        &["Meadow", "Prairie", "Field", "Clover", "Grass"]),
];

/// The fallback for a biome missing from [`FLAVORS`], so a new table row still gets names.
const FALLBACK: Flavor = flavor("", &["Lands", "Reaches", "Country"], &[], &["Far"]);

/// Adjectives that fit any biome.
#[rustfmt::skip]
const ADJECTIVES: &[&str] = &[
    "Glass", "Weeping", "Hollow", "Silent", "Amber", "Ashen", "Pale", "Black", "Gray", "Broken", "Whispering",
    "Lost", "Golden", "Iron", "Bone", "Shadow", "Starlit", "Mourning", "Wandering", "Restless", "Singing",
    "Hungry", "Old", "Last", "Far", "Copper", "Still", "Veiled", "Sleeping", "Forgotten", "Crooked", "Hidden",
    "Lonely",
];

/// First and second halves of compound names: "Ash" and "fold" make "Ashfold".
const COMPOUND_HEADS: &[&str] = &[
    "Ash", "Thorn", "Raven", "Stone", "Elder", "Grim", "Wolf", "Black", "Red", "Wind", "Bright", "Dusk", "Iron",
    "Gold", "Oak", "Hawk", "Crow", "Bram", "Ember", "Mist", "Moon", "Rook", "Heron", "Elk", "Briar", "Willow", "Alder",
    "Hart",
];
const COMPOUND_TAILS: &[&str] = &[
    "fold", "mere", "wick", "holm", "dale", "crest", "moor", "ford", "combe", "stead", "ley", "mark", "by", "ham",
    "thorpe", "hurst", "shaw", "worth", "gate", "well", "brook",
];

/// Syllabic names are a head, optionally a vowel, and a tail: "Kel" "a" "mar". Heads end in a consonant.
/// Tails that start with a vowel only follow a head directly, so vowels never pile up.
const SYLLABLE_HEADS: &[&str] = &[
    "Al", "Bel", "Cor", "Dun", "El", "Gal", "Har", "Isk", "Kel", "Lor", "Nar", "Or", "Pel", "Quel", "Ros", "Tal", "Ul",
    "Val", "Wen", "Zan", "Ar", "Cael", "Dren", "Eth", "Thal", "Ves", "Brin", "Tor",
];
const SYLLABLE_VOWELS: &[&str] = &["a", "e", "i", "o"];
const VOWEL_TAILS: &[&str] = &["an", "ar", "eth", "is", "ir", "ith", "ane", "esh", "une", "ael"];
const CONSONANT_TAILS: &[&str] = &["mar", "dor", "wen", "ra", "vel", "mir", "thas", "gard", "nor", "wyn", "dris"];

/// A stream of picks from one hash, so each choice in a name is independent of the others.
struct Draw(u64);

impl Draw {
    fn below(&mut self, n: usize) -> usize {
        self.0 = mix(self.0.wrapping_add(0x9e37_79b9_7f4a_7c15));
        // The high bits, scaled rather than taken modulo, so small lists aren't biased.
        (((self.0 >> 32) * n as u64) >> 32) as usize
    }

    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[self.below(list.len())]
    }
}

fn flavor_of(biome: BiomeId) -> &'static Flavor {
    let name = biome.biome().name;
    FLAVORS.iter().find(|f| f.biome == name).unwrap_or(&FALLBACK)
}

fn compound(draw: &mut Draw) -> String {
    format!("{}{}", draw.pick(COMPOUND_HEADS), draw.pick(COMPOUND_TAILS))
}

fn syllabic(draw: &mut Draw) -> String {
    let head = draw.pick(SYLLABLE_HEADS);
    match draw.below(3) {
        0 => {
            // A tail that repeats the head's last letters ("Har" "ar") stutters; draw again, a few times.
            let mut tail = draw.pick(VOWEL_TAILS);
            for _ in 0..4 {
                if !head.to_lowercase().ends_with(&tail[..2]) {
                    break;
                }
                tail = draw.pick(VOWEL_TAILS);
            }
            format!("{head}{tail}")
        }
        1 => format!("{head}{}", draw.pick(CONSONANT_TAILS)),
        _ => format!("{head}{}{}", draw.pick(SYLLABLE_VOWELS), draw.pick(CONSONANT_TAILS)),
    }
}

/// The placeholder name for region `id` of biome `biome` in the world for `seed`. Names starting with an
/// article keep it lowercase ("the Glass Marsh"), so they read right mid-sentence; capitalize for headings.
#[must_use]
pub fn region_name(seed: u64, id: RegionId, biome: BiomeId) -> String {
    let (x, y) = id.anchor();
    let mut draw = Draw(combine(combine(combine(derive(seed, b"regnames"), x as u64), y as u64), u64::from(biome.0)));
    let flavor = flavor_of(biome);
    let noun = draw.pick(flavor.nouns);
    match draw.below(20) {
        0..9 => {
            let own = !flavor.adjectives.is_empty() && draw.below(2) == 0;
            let adjective = draw.pick(if own { flavor.adjectives } else { ADJECTIVES });
            format!("the {adjective} {noun}")
        }
        9..14 => format!("{} {noun}", compound(&mut draw)),
        14..17 => format!("{} {noun}", syllabic(&mut draw)),
        _ => {
            let proper = if draw.below(2) == 0 { compound(&mut draw) } else { syllabic(&mut draw) };
            format!("the {noun} of {proper}")
        }
    }
}

/// One family's possible clade roots. Each family has its own list, so roots never collide within a world.
struct Roots {
    family: Family,
    roots: &'static [&'static str],
}

#[rustfmt::skip]
const ROOTS: &[Roots] = &[
    Roots { family: Family::Crawler, roots: &["Skitter", "Clicker", "Scuttler", "Creeper", "Borer", "Tickler"] },
    Roots { family: Family::Serpent, roots: &["Coil", "Slither", "Asp", "Hooder", "Winder", "Hisser"] },
    Roots { family: Family::Beast,   roots: &["Prowler", "Gnasher", "Mauler", "Snuffler", "Brute", "Grizzle"] },
    Roots { family: Family::Flier,   roots: &["Flitter", "Swoop", "Glider", "Screecher", "Wingling", "Kite"] },
    Roots { family: Family::Blob,    roots: &["Gloop", "Ooze", "Wobbler", "Jelly", "Quiver", "Blub"] },
    Roots { family: Family::Crab,    roots: &["Pincher", "Clacker", "Snapper", "Nipper", "Sidler", "Clawback"] },
    Roots { family: Family::Toad,    roots: &["Croaker", "Gulper", "Hopper", "Bloat", "Puffer", "Warty"] },
    Roots { family: Family::Ray,     roots: &["Glide", "Skate", "Lash", "Manta", "Drifter", "Whiptail"] },
];

/// The root name of `family`'s clade in the world for `seed`.
#[must_use]
pub fn clade_root(seed: u64, family: Family) -> &'static str {
    let roots = ROOTS.iter().find(|r| r.family == family).map_or(&["Thing"][..], |r| r.roots);
    Draw(combine(derive(seed, b"cladroot"), family as u64)).pick(roots)
}

/// The placeholder name of the species of clade `root` in region `id` of biome `biome`: the region's
/// epithet and the root, as in "Dune Skitter".
#[must_use]
pub fn species_name(seed: u64, id: RegionId, biome: BiomeId, root: &str) -> String {
    let (x, y) = id.anchor();
    let mut draw = Draw(combine(combine(derive(seed, b"epithets"), x as u64), y as u64));
    format!("{} {root}", draw.pick(flavor_of(biome).epithets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::BIOMES;

    #[test]
    fn every_biome_has_a_flavor() {
        for row in BIOMES {
            assert_eq!(FLAVORS.iter().filter(|f| f.biome == row.name).count(), 1, "{}", row.name);
        }
        assert_eq!(FLAVORS.len(), BIOMES.len());
    }

    #[test]
    fn name_parts_never_repeat_a_noun() {
        // A proper name built around a noun ("Fenmar Fens") reads as a stutter.
        let nouns: Vec<String> = FLAVORS.iter().flat_map(|f| f.nouns).map(|n| n.to_lowercase()).collect();
        for part in COMPOUND_HEADS.iter().chain(COMPOUND_TAILS).chain(SYLLABLE_HEADS) {
            let part = part.to_lowercase();
            assert!(!nouns.contains(&part), "{part} is also a noun");
        }
    }

    #[test]
    fn species_names_pair_an_epithet_with_the_root() {
        for flavor in FLAVORS {
            assert!(!flavor.epithets.is_empty(), "{}", flavor.biome);
            assert!(flavor.epithets.iter().all(|e| !e.contains(' ')), "{}", flavor.biome);
        }
        for roots in ROOTS {
            assert!(roots.roots.iter().all(|r| !r.contains(' ')));
        }
        assert_eq!(ROOTS.len(), Family::ALL.len());
        let id = crate::World::new(3).region_at(0, 0);
        let desert = BiomeId::all().find(|b| b.biome().name == "Desert").unwrap();
        let name = species_name(3, id, desert, "Skitter");
        assert!(name.ends_with(" Skitter") && name.split(' ').count() == 2, "{name}");
    }

    #[test]
    fn draws_cover_their_range() {
        let mut draw = Draw(7);
        let mut seen = [false; 5];
        for _ in 0..200 {
            seen[draw.below(5)] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
