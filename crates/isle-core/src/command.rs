//! Typed commands, Kesmai style: a verb, sometimes a target, and abbreviations wherever they are unambiguous.

use crate::data::Spell;

/// One of the eight ways to step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Up the screen.
    N,
    /// Down the screen.
    S,
    /// Right.
    E,
    /// Left.
    W,
    /// Up and right.
    Ne,
    /// Up and left.
    Nw,
    /// Down and right.
    Se,
    /// Down and left.
    Sw,
}

impl Dir {
    /// The cell offset of one step.
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::S => (0, 1),
            Dir::E => (1, 0),
            Dir::W => (-1, 0),
            Dir::Ne => (1, -1),
            Dir::Nw => (-1, -1),
            Dir::Se => (1, 1),
            Dir::Sw => (-1, 1),
        }
    }

    /// The compass name, short.
    pub const fn short(self) -> &'static str {
        match self {
            Dir::N => "n",
            Dir::S => "s",
            Dir::E => "e",
            Dir::W => "w",
            Dir::Ne => "ne",
            Dir::Nw => "nw",
            Dir::Se => "se",
            Dir::Sw => "sw",
        }
    }

    fn parse(word: &str) -> Option<Dir> {
        Some(match word {
            "n" | "north" => Dir::N,
            "s" | "south" => Dir::S,
            "e" | "east" => Dir::E,
            "w" | "west" => Dir::W,
            "ne" | "northeast" => Dir::Ne,
            "nw" | "northwest" => Dir::Nw,
            "se" | "southeast" => Dir::Se,
            "sw" | "southwest" => Dir::Sw,
            _ => return None,
        })
    }

    /// The direction of a one-cell offset, if it is one.
    pub fn from_delta(dx: i32, dy: i32) -> Option<Dir> {
        Some(match (dx.signum(), dy.signum()) {
            (0, -1) => Dir::N,
            (0, 1) => Dir::S,
            (1, 0) => Dir::E,
            (-1, 0) => Dir::W,
            (1, -1) => Dir::Ne,
            (-1, -1) => Dir::Nw,
            (1, 1) => Dir::Se,
            (-1, 1) => Dir::Sw,
            _ => return None,
        })
    }
}

/// Who a command is aimed at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The letter the creature list shows.
    Label(char),
    /// The `nth` creature in sight whose name has a word starting with `name`, nearest first.
    Named {
        /// One-based position among matches.
        nth: usize,
        /// What was typed.
        name: String,
    },
}

/// A command that takes a round to carry out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Up to three steps.
    Move(Vec<Dir>),
    /// Strike an adjacent creature with the hand weapon.
    Attack(Option<Target>),
    /// Fit an arrow to the bow.
    Nock,
    /// Loose the nocked arrow.
    Shoot(Option<Target>),
    /// Prepare a spell for next round.
    Chant(Spell),
    /// Release the chanted spell.
    Cast(Option<Target>),
    /// Slip into the shadows.
    Hide,
    /// Pick up something underfoot, or `all`.
    Get(String),
    /// Put something down.
    Drop(String),
    /// Take a weapon or bow in hand.
    Wield(String),
    /// Put on armor or the ring.
    Wear(String),
    /// Drink a healing potion.
    Quaff,
    /// Catch your breath.
    Rest,
    /// Take the stairs down.
    Down,
    /// Take the stairs up.
    Up,
    /// Buy from the outfitter.
    Buy(String),
    /// Sell to the outfitter.
    Sell(String),
    /// Ask the temple to heal you.
    Heal,
    /// Do nothing this round.
    Wait,
}

/// A command answered at once, without spending a round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instant {
    /// Describe where you stand and what you see.
    Look,
    /// List what you carry.
    Inventory,
    /// The outfitter's wares.
    List,
    /// The commands.
    Help,
}

/// What a typed line turned into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// Queue for the next round.
    Round(Command),
    /// Answer now.
    Instant(Instant),
}

#[derive(Clone, Copy)]
enum Verb {
    Attack,
    Nock,
    Shoot,
    Chant,
    Cast,
    Hide,
    Get,
    Drop,
    Wield,
    Wear,
    Quaff,
    Rest,
    Down,
    Up,
    Buy,
    Sell,
    Heal,
    Wait,
    Look,
    Inventory,
    List,
    Help,
}

/// Verbs with the shortest prefix each accepts. Earlier entries win, so `we` is wear and `w` is west.
const VERBS: [(&str, usize, Verb); 27] = [
    ("attack", 1, Verb::Attack),
    ("fight", 1, Verb::Attack),
    ("kill", 1, Verb::Attack),
    ("nock", 2, Verb::Nock),
    ("shoot", 2, Verb::Shoot),
    ("chant", 2, Verb::Chant),
    ("cast", 2, Verb::Cast),
    ("hide", 2, Verb::Hide),
    ("get", 1, Verb::Get),
    ("take", 2, Verb::Get),
    ("drop", 2, Verb::Drop),
    ("wield", 2, Verb::Wield),
    ("wear", 2, Verb::Wear),
    ("quaff", 1, Verb::Quaff),
    ("drink", 3, Verb::Quaff),
    ("rest", 1, Verb::Rest),
    ("down", 1, Verb::Down),
    ("up", 1, Verb::Up),
    ("buy", 1, Verb::Buy),
    ("sell", 2, Verb::Sell),
    ("heal", 3, Verb::Heal),
    ("wait", 2, Verb::Wait),
    ("look", 1, Verb::Look),
    ("inventory", 1, Verb::Inventory),
    ("list", 2, Verb::List),
    ("help", 3, Verb::Help),
    ("?", 1, Verb::Help),
];

/// Steps one command may take.
pub const MAX_STEPS: usize = 3;

/// Reads one command. The error is a line for the log.
pub fn parse(line: &str) -> Result<Parsed, String> {
    let lower = line.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let Some(&first) = words.first() else {
        return Ok(Parsed::Round(Command::Wait));
    };
    if let Some(steps) = words.iter().map(|w| Dir::parse(w)).collect::<Option<Vec<_>>>() {
        if steps.len() > MAX_STEPS {
            return Err("You can take at most three steps in a round.".into());
        }
        return Ok(Parsed::Round(Command::Move(steps)));
    }
    let verb = VERBS
        .iter()
        .find(|(name, min, _)| first.len() >= *min && name.starts_with(first))
        .map(|&(_, _, verb)| verb)
        .ok_or_else(|| format!("I don't know how to \"{first}\". Type help for the commands."))?;
    let rest = &words[1..];
    let object = || rest.join(" ");
    let target = || parse_target(rest);
    let need = |what: &str| -> Result<String, String> {
        if rest.is_empty() { Err(format!("{} what?", capitalized(what))) } else { Ok(object()) }
    };
    Ok(Parsed::Round(match verb {
        Verb::Attack => Command::Attack(target()),
        Verb::Nock => Command::Nock,
        Verb::Shoot => Command::Shoot(target()),
        Verb::Chant => {
            let name = need("chant")?;
            let spell = Spell::ALL
                .into_iter()
                .find(|s| s.name().starts_with(&name))
                .ok_or_else(|| format!("You know no spell called \"{name}\"."))?;
            Command::Chant(spell)
        }
        Verb::Cast => Command::Cast(target()),
        Verb::Hide => Command::Hide,
        Verb::Get => Command::Get(need("get")?),
        Verb::Drop => Command::Drop(need("drop")?),
        Verb::Wield => Command::Wield(need("wield")?),
        Verb::Wear => Command::Wear(need("wear")?),
        Verb::Quaff => Command::Quaff,
        Verb::Rest => Command::Rest,
        Verb::Down => Command::Down,
        Verb::Up => Command::Up,
        Verb::Buy => Command::Buy(need("buy")?),
        Verb::Sell => Command::Sell(need("sell")?),
        Verb::Heal => Command::Heal,
        Verb::Wait => Command::Wait,
        Verb::Look => return Ok(Parsed::Instant(Instant::Look)),
        Verb::Inventory => return Ok(Parsed::Instant(Instant::Inventory)),
        Verb::List => return Ok(Parsed::Instant(Instant::List)),
        Verb::Help => return Ok(Parsed::Instant(Instant::Help)),
    }))
}

fn parse_target(words: &[&str]) -> Option<Target> {
    let words: Vec<&str> = words.iter().copied().filter(|w| *w != "at" && *w != "the").collect();
    match words.as_slice() {
        [] => None,
        [one] if one.len() == 1 && one.chars().all(|c| c.is_ascii_alphabetic()) => {
            Some(Target::Label(one.chars().next()?.to_ascii_uppercase()))
        }
        [n, name @ ..] if !name.is_empty() && n.parse::<usize>().is_ok_and(|n| n > 0) => {
            Some(Target::Named { nth: n.parse().ok()?, name: name.join(" ") })
        }
        name => Some(Target::Named { nth: 1, name: name.join(" ") }),
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// The help text, one log line each.
pub const HELP: [&str; 10] = [
    "Move: n s e w ne nw se sw, up to three a round (n n e). Rounds tick on their own.",
    "Fight: attack <who>, nock then shoot <who>. Who is a letter (shoot a) or a name (sh 2 orc).",
    "Magic: chant <spell>, then cast [who]. Wizard: spark, shield. Thaumaturge: heal, stun.",
    "Thieves: hide, then strike or shoot from hiding for extra damage.",
    "Things: get <item>|all, drop, wield, wear, quaff, inventory, look.",
    "Town: buy <item>, sell <item>, list (in the shop); heal (in the temple).",
    "Stairs: down, up. Rest to recover, wait to stand.",
    "Chain rounds with ;  (nock; shoot a). Enter on an empty line repeats your last command.",
    "Up arrow recalls it. pause stops the clock, stop clears queued rounds.",
    "Commands abbreviate: a, sh, no, ch, ca, g, l, i, q, r, d, u.",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn round(line: &str) -> Command {
        match parse(line) {
            Ok(Parsed::Round(c)) => c,
            other => panic!("{line:?} gave {other:?}"),
        }
    }

    #[test]
    fn kesmai_phrases_parse() {
        assert_eq!(round("nock bow"), Command::Nock);
        assert_eq!(round("shoot orc"), Command::Shoot(Some(Target::Named { nth: 1, name: "orc".into() })));
        assert_eq!(round("sh 2 orc"), Command::Shoot(Some(Target::Named { nth: 2, name: "orc".into() })));
        assert_eq!(round("shoot at the b"), Command::Shoot(Some(Target::Label('B'))));
        assert_eq!(round("a"), Command::Attack(None));
        assert_eq!(round("get all"), Command::Get("all".into()));
        assert_eq!(round("ch sp"), Command::Chant(Spell::Spark));
    }

    #[test]
    fn short_words_resolve_by_order() {
        assert_eq!(round("n n e"), Command::Move(vec![Dir::N, Dir::N, Dir::E]));
        assert_eq!(round("se"), Command::Move(vec![Dir::Se]));
        assert_eq!(round("we leather"), Command::Wear("leather".into()));
        assert_eq!(round("west"), Command::Move(vec![Dir::W]));
        assert_eq!(round("no"), Command::Nock);
        assert_eq!(round("se sword"), Command::Sell("sword".into()));
        assert_eq!(round("d"), Command::Down);
        assert_eq!(round("dr dagger"), Command::Drop("dagger".into()));
        assert_eq!(parse("l"), Ok(Parsed::Instant(Instant::Look)));
        assert_eq!(parse("i"), Ok(Parsed::Instant(Instant::Inventory)));
    }

    #[test]
    fn bad_lines_explain_themselves() {
        assert!(parse("n n n n").unwrap_err().contains("three"));
        assert!(parse("xyzzy").unwrap_err().contains("xyzzy"));
        assert_eq!(parse("get").unwrap_err(), "Get what?");
        assert!(parse("chant fireball").unwrap_err().contains("fireball"));
    }
}
