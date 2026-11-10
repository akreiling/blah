#!/usr/bin/env python3
"""Ask Jev narrow, typed questions about the proposed loadout rules and combine the answers in code.

Usage:
    python3 scripts/jev_loadout_review.py [--dry-run]

Writes raw responses and a summary under docs/review/jev/. Jev judges what the written rules imply.
Its answers are calibrated opinions about the text, not playtests, balance measurements, or proof of fun.
"""

import argparse
import datetime
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import jev  # noqa: E402

OUT = jev.ROOT / "docs" / "review" / "jev"

RULES = {
    "game": "Turn-based, grid-based roguelike RPG. Classless: characters grow by what they do. "
    "Ecology-driven world with regional creatures whose traits vary.",
    "design_goals": [
        "Every item property makes some things harder and others better.",
        "No list of allowed combinations; builds come from properties and the actions that read them.",
        "Costly specialized power is fine; cost-free universal dominance is not.",
        "Losing a fight is fine; retreat and dying are legitimate ways out.",
    ],
    "properties": {
        "mass": "light/medium/heavy. Harder: sneaking, dodging, swimming (heavy sinks in deep water), "
        "travel in heat. Better: charges, bashes, resisting knockback, heavy-weapon cleave.",
        "noise": "quiet/normal/loud. Harder: sneaking, ambushes, avoiding notice. "
        "Better: drawing enemies onto yourself to protect someone, scaring off weak creatures.",
        "material": "cloth/leather/wood/metal. Metal: fire heats it and burns its wearer; lightning arcs "
        "through it; metal holds inscribed wards well; metal resists channeling unless warded. "
        "Wood carries channeled magic well. Fire burns cloth.",
        "rigidity": "flexible/stiff. Stiff: blocks martial arts and acrobatics, but blades glance off.",
        "size": "small/large weapons. Small: little reach or force, but concealable and suits backstabs.",
        "combining": "A loadout's property value is mostly its loudest, heaviest, or stiffest piece.",
    },
    "items": {
        "dagger": "weapon; light, quiet, metal, small. Intended: quiet kills from hiding.",
        "staff": "weapon; medium, normal noise, wood, large. Magic focus or martial weapon.",
        "spear": "weapon; medium, normal noise, wood and metal, large. Reach and piercing.",
        "greatsword": "weapon; heavy, loud, metal, large. Straight fights, several enemies.",
        "robe": "armor; light, quiet, cloth, flexible. Movement and casting.",
        "leather": "armor; light, quiet, leather, flexible. Some protection, keeps stealth.",
        "plate": "armor; heavy, loud, metal, stiff. Maximum protection and mass.",
    },
    "actions": {
        "basic_strike": "Always available, never costs Focus. Every weapon can win by beating an enemy down.",
        "sneak": "Only with a quiet, not-heavy loadout.",
        "backstab": "Small weapon, while sneaking, from behind an unaware creature: a large hit. Creatures face "
        "the way they last moved, or toward you once they notice you.",
        "martial_arts": "Staff or empty hands with flexible armor: sweeps, trips, parries.",
        "cleave": "A heavy weapon hits every adjacent enemy.",
        "charge": "Moving into an enemy hits harder the heavier you are; heavy targets resist being moved.",
        "casting": "Wood and cloth carry magic freely; metal resists channeling unless warded. "
        "Magic costs Focus. Focus is restored at a safe home or by death.",
        "wards": "Protective magic inscribed into gear; holds best in metal.",
    },
    "world": [
        "Desert heat wears down heavy and metal gear wearers.",
        "Tundra: metal is cold; cloth alone is not enough.",
        "Deep water or swamp: heavy gear sinks; plate can drown you.",
        "Fire-breathing creatures punish metal armor.",
        "Clawed or biting creatures reward rigid armor.",
        "Keen-eared creatures hear even normal noise.",
        "Death returns you home with full Health and Focus after a 1,000-turn time penalty.",
    ],
}

SITUATIONS = {
    "ambush_unaware": "approaching an unaware creature to attack it first",
    "straight_fight_clawed": "a straight fight against one clawed creature",
    "surrounded": "being surrounded by several weak creatures",
    "fire_breather": "fighting a fire-breathing creature",
    "deep_water": "crossing deep water or a swamp",
    "desert_travel": "a long journey through desert heat",
    "tundra": "travelling through a frozen tundra",
    "keen_ears": "avoiding a creature with keen hearing",
    "protect_ally": "drawing enemies away from a weaker companion",
    "flee_pursuer": "escaping a pursuer of equal speed",
    "cast_fire": "casting fire magic",
    "cast_wards": "casting protective wards",
    "lightning": "being struck by lightning magic",
    "knockback": "being shoved toward a cliff or into water",
    "armored_foe": "fighting a heavily armored creature",
    "long_fight": "a long fight where Focus runs out",
}


def matrix_request(item: str) -> dict:
    questions = {}
    for key, text in SITUATIONS.items():
        for side, word in (("worse", "worse"), ("better", "better")):
            questions[f"{key}__{side}"] = {
                "type": "noul",
                "instructions": f"Under these rules, does using the {item} make this situation {word} for its "
                f"user than a typical alternative in the same slot? Situation: {text}.",
                "criteria": {
                    "true": f"The rules clearly make it {word} because of the {item}'s properties.",
                    "false": f"The rules do not make it {word}, or the {item} is irrelevant here.",
                },
            }
    return {"state": {"rules": RULES, "item": RULES["items"][item]}, "questions": questions}


VIABILITY = ["Not viable", "Narrow niche", "Solid", "Dominant everywhere"]
LOADOUTS = {
    "dagger_leather": "dagger with leather armor",
    "greatsword_plate": "greatsword with plate armor",
    "staff_robe_martial": "staff with a robe, fighting with martial arts",
    "staff_robe_fire": "staff with a robe, casting fire magic",
    "sword_leather_magic": "a sword with leather armor, plus subtle magic",
    "spear_plate_wards": "spear with plate armor and wards",
    "plate_wards_force_charge": "plate armor, wards, force magic, and heavy charges",
    "dagger_plate": "dagger with plate armor (unlisted)",
    "greatsword_robe": "greatsword with a robe (unlisted)",
    "staff_plate_wards": "staff with plate armor and wards (unlisted)",
    "spear_leather": "spear with leather armor (unlisted)",
    "empty_hands_robe": "empty hands with a robe, martial arts only (unlisted)",
}

PATTERNS = {
    "wards_cancel_metal": "Do wards remove the only real cost of wearing metal for a caster, making plate casters "
    "strictly better than cloth casters?",
    "plate_force_uncountered": "Is plate + wards + force magic + heavy charges a combination with no real counter "
    "among the listed rules and environments?",
    "staff_strictly_better": "Does the staff's dual use (magic focus and martial weapon) make it strictly better than "
    "every other weapon for any character in a robe?",
    "heaviest_piece_exploit": "Does the rule that a loadout's value is mostly its loudest, heaviest, or stiffest piece "
    "invite strange or exploitable mixes, such as one plate piece or swapping gear mid-fight?",
    "backstab_trivial": "With facing defined as the last move direction, or toward you once noticed, can a player "
    "reliably get behind creatures with a simple routine, making backstabs trivial?",
    "backstab_too_rare": "Could backstabs become so hard that the dagger has no useful role?",
    "death_exploit": "Can a player profit from dying on purpose (spend everything, die, return restored), given the "
    "1,000-turn penalty and nothing stated about the world changing with time?",
    "focus_optional": "With a free basic strike always available, does spending Focus on magic become optional to "
    "the point that magic is a side feature?",
    "sneak_binary": "Is sneaking described as all-or-nothing, so normal-noise gear can never sneak at all?",
    "rules_ambiguous": "Are any action rules too vague to implement without guessing (for example how strongly "
    "metal resists channeling)?",
}

MISSING = {
    "time_pressure": "World changes over time, so penalties and waiting matter",
    "gear_swapping": "When and how gear can be changed",
    "magic_counters": "What protects against magic other than material",
    "detection": "How creatures notice noise and sight in detail",
    "focus_economy": "How Focus is earned and spent beyond home and death",
    "ranged": "Ranged attacks and how they read properties",
}


def review_request() -> dict:
    questions = {}
    for key, text in LOADOUTS.items():
        questions[f"viability__{key}"] = {
            "type": "score",
            "instructions": f"Under these rules, how viable is a character using {text} across the listed "
            "situations and world conditions?",
            "criteria": VIABILITY,
        }
    for key, text in PATTERNS.items():
        questions[f"pattern__{key}"] = {
            "type": "noul",
            "instructions": text,
            "criteria": {"true": "Yes, the rules as written allow or imply this.", "false": "No, the rules prevent it."},
        }
    questions["missing_rule"] = {
        "type": "choice",
        "instructions": "Which missing rule would most change whether these rules avoid a dominant loadout?",
        "criteria": MISSING,
    }
    return {"state": {"rules": RULES}, "questions": questions}


def send(name: str, request: dict, dry: bool) -> dict | None:
    tokens = jev.estimate_tokens({"model": jev.MODEL, **request})
    print(f"{name}: {len(request['questions'])} questions, ~{tokens} tokens", file=sys.stderr)
    if dry:
        return None
    return jev.decide(request)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    raw, cost = {}, 0.0
    for item in RULES["items"]:
        raw[f"matrix_{item}"] = send(f"matrix {item}", matrix_request(item), args.dry_run)
    raw["review"] = send("review", review_request(), args.dry_run)
    if args.dry_run:
        return
    model = raw["review"]["model"]
    cost = sum(r.get("usage", {}).get("cost", 0) for r in raw.values())
    OUT.mkdir(parents=True, exist_ok=True)
    stamp = datetime.date.today().isoformat()
    (OUT / f"loadout-review-{stamp}.json").write_text(json.dumps(raw, indent=2) + "\n")
    (OUT / f"loadout-review-{stamp}.md").write_text(summarize(raw, model, cost, stamp))
    print(f"wrote {OUT}/loadout-review-{stamp}.{{json,md}}; total ${cost:.6f}", file=sys.stderr)


YES = 0.6


def summarize(raw: dict, model: str, cost: float, stamp: str) -> str:
    lines = [
        f"# Jev loadout review, {stamp}",
        "",
        "[Review process](../README.md) · [Loadout properties](../../systems/loadout-properties.md)",
        "",
        f"Generated by `scripts/jev_loadout_review.py` with `{model}`. Total cost ${cost:.4f}.",
        "Jev judged what the written rules imply. These are calibrated opinions about the text, not",
        f"playtests or balance measurements. A probability of {YES} or more counts as yes.",
        "",
        "## Does every item cut both ways?",
        "",
        "| Item | Worse in | Better in | Downsides | Upsides |",
        "| --- | ---: | ---: | --- | --- |",
    ]
    table = {}
    for item in RULES["items"]:
        answers = raw[f"matrix_{item}"]["answers"]
        p = {k: v["noul"] for k, v in answers.items()}
        table[item] = p
        worse = [s for s in SITUATIONS if p[f"{s}__worse"] >= YES]
        better = [s for s in SITUATIONS if p[f"{s}__better"] >= YES]
        lines.append(
            f"| {item} | {len(worse)} | {len(better)} | {', '.join(worse) or '**none**'} | "
            f"{', '.join(better) or '**none**'} |"
        )
    lines += ["", "Items with no situation on one side fail the both-ways rule as written.", ""]

    lines += ["## Dominance within a slot", ""]
    slots = {
        "weapon": ["dagger", "staff", "spear", "greatsword"],
        "armor": ["robe", "leather", "plate"],
    }
    found = False
    for members in slots.values():
        for a in members:
            for b in members:
                if a == b:
                    continue
                if all(
                    table[a][f"{s}__worse"] <= table[b][f"{s}__worse"] + 0.05
                    and table[a][f"{s}__better"] >= table[b][f"{s}__better"] - 0.05
                    for s in SITUATIONS
                ):
                    found = True
                    lines.append(f"- **{a}** is never worse and never less helpful than **{b}** in any situation.")
    if not found:
        lines.append("No item dominates another in its slot across all situations.")
    lines.append("")

    review = raw["review"]["answers"]
    lines += ["## Loadout viability", "", "| Loadout | Score (0 to 3) | Most likely |", "| --- | ---: | --- |"]
    for key, text in LOADOUTS.items():
        a = review[f"viability__{key}"]
        likely = VIABILITY[max(range(len(VIABILITY)), key=lambda i: a["probabilities"][str(i)])]
        lines.append(f"| {text} | {a['score']:.2f} | {likely} |")
    lines += ["", "## Risk patterns", "", "| Pattern | Probability yes |", "| --- | ---: |"]
    for key, text in sorted(PATTERNS.items(), key=lambda kv: -review[f"pattern__{kv[0]}"]["noul"]):
        lines.append(f"| {text} | {review[f'pattern__{key}']['noul']:.2f} |")
    m = review["missing_rule"]
    lines += [
        "",
        "## Most important missing rule",
        "",
        f"**{MISSING[m['choice']]}** (confidence {m.get('confidence', 0):.2f}).",
        "",
        "| Option | Probability |",
        "| --- | ---: |",
    ]
    for key, p in sorted(m["probabilities"].items(), key=lambda kv: -kv[1]):
        lines.append(f"| {MISSING[key]} | {p:.2f} |")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
