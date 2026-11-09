//! Placeholder names for regions and species: the offline fallback until a remote model renames them, or
//! forever.
//!
//! Every name is a pure function of the seed and what it names, drawn from the word lists below. All of
//! the wording lives in this file as data.
//!
//! A species name is a biome epithet and its clade's root: "Dune Skitter", "Fen Skitter". The root says
//! which lineage it is and the epithet where it lives, so the relationship shows in the name. Every
//! species of one region shares the region's epithet. Each biome has its own epithets, never shared with
//! another, and enough of them that two regions of one biome rarely give a clade the same name.
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
use crate::hash::Rng;
use crate::region::RegionId;

/// One biome's flavor: what to call a place of this kind, what places of this kind are like, and what to
/// call the things that live there.
struct Flavor {
    /// Must equal a name in [`BIOMES`](crate::BIOMES); `every_biome_has_a_flavor` checks.
    biome: &'static str,
    nouns: &'static [&'static str],
    adjectives: &'static [&'static str],
    /// First words of species names. Single words, so "Dune Skitter" stays two, and never another biome's.
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
        &["Abyss", "Deep", "Brine", "Gulf", "Tide", "Fathom", "Trench", "Murk",
          "Kraken", "Undertow", "Leviathan", "Pelagic", "Swell", "Current", "Gloom", "Benthic"]),
    flavor("Shallow water",
        &["Shallows", "Shoals", "Sound", "Bay", "Lagoon", "Strait", "Reach"],
        &["Clear", "Tidal", "Rippling", "Sandy", "Bright", "Wading"],
        &["Shoal", "Reef", "Lagoon", "Surf", "Kelp", "Coral", "Eelgrass", "Ripple",
          "Sandbar", "Inlet", "Cove", "Spray", "Wrack", "Seagrass", "Tidepool", "Brack"]),
    flavor("Snow peaks",
        &["Peaks", "Spires", "Crowns", "Horns", "Teeth", "Summits"],
        &["White", "Frozen", "Frostbitten", "Howling", "Icebound", "Shining"],
        &["Peak", "Ice", "Summit", "Glacier", "Avalanche", "Cornice", "Serac", "Icefall",
          "Whiteout", "Blizzard", "Aerie", "Crevasse", "Spindrift", "Snowcap", "Iceblink", "Pinnacle"]),
    flavor("Mountains",
        &["Mountains", "Crags", "Heights", "Range", "Tors", "Ridges"],
        &["Jagged", "Stern", "Thundering", "Granite", "High", "Shattered"],
        &["Crag", "Tor", "Shale", "Ridge", "Granite", "Cliff", "Boulder", "Talus",
          "Basalt", "Gorge", "Ledge", "Flint", "Col", "Cairn", "Bluff", "Slate"]),
    flavor("Badlands",
        &["Badlands", "Breaks", "Scarps", "Mesas", "Wastes", "Gullies"],
        &["Scorched", "Red", "Cracked", "Rust", "Bleached", "Crumbling"],
        &["Mesa", "Rust", "Gully", "Scarp", "Cinder", "Butte", "Hoodoo", "Arroyo",
          "Ochre", "Clinker", "Redrock", "Canyon", "Gulch", "Slag", "Brimstone", "Sinter"]),
    flavor("Hills",
        &["Hills", "Downs", "Rise", "Knolls", "Fells", "Uplands"],
        &["Rolling", "Windswept", "Gentle", "Barrow", "Folded", "Tumbled"],
        &["Hill", "Down", "Fell", "Barrow", "Knoll", "Heath", "Brae", "Wold",
          "Dell", "Hillock", "Gorse", "Hedge", "Chalk", "Sheep", "Bramble", "Lark"]),
    flavor("Tundra",
        &["Tundra", "Barrens", "Steppe", "Flats", "Wastes", "Expanse"],
        &["Frozen", "White", "Frostbitten", "Windswept", "Pale", "Bitter"],
        &["Frost", "Rime", "Snow", "Tundra", "Permafrost", "Hoarfrost", "Lichen", "Sedge",
          "Musk", "Sleet", "Polar", "Hail", "Icewind", "Frostbite", "Barren", "Cottongrass"]),
    flavor("Beach",
        &["Strand", "Shore", "Sands", "Coast", "Shingle", "Spit"],
        &["Bright", "Shell", "Driftwood", "Gull", "Tidewashed", "Pearl"],
        &["Shore", "Strand", "Shingle", "Shell", "Dune", "Driftwood", "Pebble", "Tideline",
          "Gull", "Sandspit", "Seafoam", "Cockle", "Barnacle", "Flotsam", "Spume", "Littoral"]),
    flavor("Taiga",
        &["Taiga", "Pinewood", "Firs", "Pines", "Spruces", "Needlewood"],
        &["Snowbound", "Hushed", "Frosted", "Dark", "Weeping", "Resin"],
        &["Pine", "Spruce", "Needle", "Fir", "Resin", "Larch", "Boreal", "Pinecone",
          "Hemlock", "Muskeg", "Bough", "Tamarack", "Pitch", "Taiga", "Duff", "Lynx"]),
    flavor("Swamp",
        &["Marsh", "Fen", "Fens", "Mire", "Bog", "Swamp", "Slough"],
        &["Sodden", "Sunken", "Murky", "Misty", "Reedy", "Weeping"],
        &["Fen", "Bog", "Mire", "Marsh", "Reed", "Peat", "Cattail", "Bulrush",
          "Quag", "Silt", "Sludge", "Cypress", "Mangrove", "Bayou", "Morass", "Slough"]),
    flavor("Desert",
        &["Desert", "Sands", "Dunes", "Erg", "Waste", "Barrens"],
        &["Burning", "Thirsting", "Bleached", "Shimmering", "Scorched", "Endless"],
        &["Sand", "Sun", "Scorch", "Dust", "Erg", "Mirage", "Sirocco", "Oasis",
          "Glare", "Sandstorm", "Thirst", "Saltpan", "Scarab", "Hardpan", "Simoom", "Heatwave"]),
    flavor("Savanna",
        &["Savanna", "Veld", "Plains", "Grasslands", "Scrub"],
        &["Golden", "Sunlit", "Dusty", "Thorn", "Tawny", "Baking"],
        &["Veld", "Thorn", "Acacia", "Tawny", "Baobab", "Grassfire", "Termite", "Kopje",
          "Bush", "Scrub", "Waterhole", "Mopane", "Sunbake", "Gazelle", "Wildgrass", "Dryland"]),
    flavor("Jungle",
        &["Jungle", "Tangle", "Wilds", "Thicket", "Canopy", "Rainwood"],
        &["Green", "Tangled", "Steaming", "Humming", "Verdant", "Vine"],
        &["Vine", "Canopy", "Jungle", "Orchid", "Liana", "Monsoon", "Bromeliad", "Strangler",
          "Palm", "Frond", "Teak", "Rattan", "Bamboo", "Kapok", "Understory", "Mahogany"]),
    flavor("Forest",
        &["Forest", "Wood", "Weald", "Holt", "Woods", "Greenwood"],
        &["Elder", "Mossy", "Deep", "Shaded", "Oaken", "Dim"],
        &["Wood", "Oak", "Moss", "Bracken", "Glade", "Beech", "Elm", "Hazel",
          "Acorn", "Fern", "Copse", "Birch", "Holly", "Ivy", "Rowan", "Toadstool"]),
    flavor("Grassland",
        &["Plains", "Meadows", "Fields", "Prairie", "Leas", "Grasslands"],
        &["Open", "Wide", "Long", "Gentle", "Wildflower", "Swaying"],
        &["Meadow", "Prairie", "Field", "Clover", "Grass", "Thistle", "Buttercup", "Bluestem",
          "Sward", "Hay", "Daisy", "Tallgrass", "Sorrel", "Yarrow", "Vetch", "Pasture"]),
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

/// One word from `list`, drawn from `rng`. Each choice in a name is its own draw, so they are independent.
fn pick<'a>(rng: &mut Rng, list: &[&'a str]) -> &'a str {
    list[rng.below(list.len())]
}

fn flavor_of(biome: BiomeId) -> &'static Flavor {
    let name = biome.biome().name;
    FLAVORS.iter().find(|f| f.biome == name).unwrap_or(&FALLBACK)
}

fn compound(rng: &mut Rng) -> String {
    format!("{}{}", pick(rng, COMPOUND_HEADS), pick(rng, COMPOUND_TAILS))
}

fn syllabic(rng: &mut Rng) -> String {
    let head = pick(rng, SYLLABLE_HEADS);
    match rng.below(3) {
        0 => {
            // A tail that repeats the head's last letters ("Har" "ar") stutters; draw again, a few times.
            let mut tail = pick(rng, VOWEL_TAILS);
            for _ in 0..4 {
                if !head.to_lowercase().ends_with(&tail[..2]) {
                    break;
                }
                tail = pick(rng, VOWEL_TAILS);
            }
            format!("{head}{tail}")
        }
        1 => format!("{head}{}", pick(rng, CONSONANT_TAILS)),
        _ => format!("{head}{}{}", pick(rng, SYLLABLE_VOWELS), pick(rng, CONSONANT_TAILS)),
    }
}

/// The placeholder name for region `id` of biome `biome` in the world for `seed`. Names starting with an
/// article keep it lowercase ("the Glass Marsh"), so they read right mid-sentence; capitalize for headings.
#[must_use]
pub fn region_name(seed: u64, id: RegionId, biome: BiomeId) -> String {
    let (x, y) = id.anchor();
    let mut rng = Rng::keyed(seed, b"regnames", &[x as u64, y as u64, u64::from(biome.0)]);
    let flavor = flavor_of(biome);
    let noun = pick(&mut rng, flavor.nouns);
    match rng.below(20) {
        0..9 => {
            let own = !flavor.adjectives.is_empty() && rng.below(2) == 0;
            let adjective = pick(&mut rng, if own { flavor.adjectives } else { ADJECTIVES });
            format!("the {adjective} {noun}")
        }
        9..14 => format!("{} {noun}", compound(&mut rng)),
        14..17 => format!("{} {noun}", syllabic(&mut rng)),
        _ => {
            let proper = if rng.below(2) == 0 { compound(&mut rng) } else { syllabic(&mut rng) };
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
    pick(&mut Rng::keyed(seed, b"cladroot", &[family as u64]), roots)
}

/// The placeholder name of the species of clade `root` in region `id` of biome `biome`: the region's
/// epithet and the root, as in "Dune Skitter".
#[must_use]
pub fn species_name(seed: u64, id: RegionId, biome: BiomeId, root: &str) -> String {
    let (x, y) = id.anchor();
    let mut rng = Rng::keyed(seed, b"epithets", &[x as u64, y as u64]);
    format!("{} {root}", pick(&mut rng, flavor_of(biome).epithets))
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
    fn epithets_are_many_and_each_biome_keeps_its_own() {
        let roots: Vec<&str> = ROOTS.iter().flat_map(|r| r.roots).copied().collect();
        let mut owner = std::collections::HashMap::new();
        for flavor in FLAVORS {
            assert!(flavor.epithets.len() >= 16, "{}: only {} epithets", flavor.biome, flavor.epithets.len());
            for &e in flavor.epithets {
                if let Some(other) = owner.insert(e, flavor.biome) {
                    panic!("{e} is an epithet of both {other} and {}", flavor.biome);
                }
                // "Ooze Ooze" or "Drift Drifter" would stutter.
                assert!(roots.iter().all(|r| !r.starts_with(e) && !e.starts_with(r)), "{e} echoes a root");
            }
        }
    }

    #[test]
    fn same_biome_regions_mostly_name_a_clade_apart() {
        // Region ids only key the draw, so any ids do; count how often two regions' names coincide.
        let world = crate::World::new(1);
        let ids: Vec<RegionId> = (0..40).map(|i| world.region_at(i * 173, i * 61 - 1_200)).collect();
        for biome in BiomeId::all() {
            let names: Vec<String> = ids.iter().map(|&id| species_name(1, id, biome, "Skitter")).collect();
            let distinct: std::collections::HashSet<_> = names.iter().collect();
            assert!(distinct.len() >= 12, "{}: {} names from 40 regions", biome.biome().name, distinct.len());
        }
    }
}
