# Progression tuning proposal

Status: proposal, not implemented. Numbers are targets to measure against, not final tuning.

## Problem

Deep 10 should take weeks of play. Today a fresh character gets there in a few thousand rounds,
because all of its power arrives early and then stops. In 32 seeded `deep_journey` runs per policy
(20,000-round cap, a stuck-free pilot), the median run reached:

| Power source | Melee pilot | Warded pilot | Depth conquered by then |
|---|---|---|---|
| Long sword (shop, 80 gold) | round 798 | round 774 | about 3 |
| Plate armor (shop, 300 gold) | round 1,463 | round 1,180 | 5 |
| Expert Blades (top rank) | round 1,551 | round 1,264 | about 5 |
| Expert Warding | n/a | round 1,996 | 7 |
| Expert Vitality | round 2,537 | round 2,070 | 6–7 |

After about round 2,500 the character cannot get stronger. Max health stops near 40. XP piles up
(median 41,000 warded, 134,000 melee) because levels give nothing. Gold piles up (6,000–7,500 by
Depth 8) because the temple heals for 5 gold and the shop's best item costs 300. Nothing gates the
stairs. The warded pilot conquers Deep 10 in a median of 4,012 rounds (28 of 32 runs); melee
characters mostly die over and over on Depths 8–10 (3 of 32 conquered it).

So Depths 6–10 are not a climb. A finished character rolls dice against harder dens.

## Goals

1. Power keeps growing all the way to Deep 10, slowly, from several sources.
2. A character that has not earned the power for a depth cannot survive there. The lockout comes
   from the danger itself, not from a locked door.
3. The surface gives real reasons to travel: wandering life, wandering encounters, and rewards
   that grow with distance from town.

## Target pace

The goal is weeks of normal play at one to two hours a day: about 20–50 hours to conquer Deep 10,
aiming for 30. Rounds per hour depend on the player's pace:

| Seconds per round | Rounds in 30 hours |
|---|---|
| 3 (fast, batched steps) | 36,000 |
| 5 | 21,600 |
| 8 (reading, careful fights) | 13,500 |

The table below assumes about 3.4 seconds per round (32,000 rounds in 30 hours). The pilot plays
faster than a person, so a real player would take somewhat longer than these round counts. The
real pace is not measured yet. The playtime log beside the save records it; after a few normal
sessions, `just playtime` gives seconds per round, and the whole table scales by it. The rounds are for the
warded pilot's median first conquest, about 8 times today's pace.

| Depth | Today (warded median round) | Target round | Ready character |
|---|---|---|---|
| 1 | 202 | 300 | level 2, practiced |
| 2 | 514 | 900 | level 4 |
| 3 | 829 | 1,800 | level 6, skilled |
| 4 | 1,047 | 3,000 | level 8 |
| 5 | 1,187 | 5,000 | level 10, adept, first deep-tier gear |
| 6 | 1,550 | 8,000 | level 12 |
| 7 | 1,895 | 12,000 | level 14, expert |
| 8 | 2,519 | 17,000 | level 16 |
| 9 | 3,185 | 24,000 | level 18, master |
| 10 | 4,012 | 32,000 | level 20, best deep-tier gear |

Acceptance gates, measured with `deep_journey` over 64 runs:

- Warded median first Deep 10 conquest is at least 28,000 rounds, which needs a 50,000-round cap.
- No depth's median comes more than 25% earlier than its target.
- Lockout: a character held at the power for depth d − 2 (fixed gear, practice, and level) wins
  at most 20% of den fights at depth d in the depth-curve benchmark, and a ready character wins
  at least 60%.
- The pilot logs no bouncing rounds, so the numbers measure the game, not the pilot.

## Levers

### 0. Fix: surface wildlife stops spawning

`populate_surface` counts every `Kind::Wild` creature in the game against the surface cap
(`crates/isle-core/src/game.rs:2572`). Deep inhabitants are also `Kind::Wild`, so a few populated
floors can fill the cap and stop all surface births. This is probably why the wandering critters
seem gone. Count only surface creatures near the player. Add a test that a populated Deep does
not stop surface births. Do this first.

### 1. Slower practice and ranks beyond Expert

Practice is earned as useful damage times threat, and deep creatures carry high threat, so ranks
come fast. Stretch the thresholds and add two ranks:

| Rank | Today | Proposed | Teaching ceiling (threat) |
|---|---|---|---|
| Novice | 0 | 0 | below 10 |
| Practiced | 1,000 | 2,000 | 10 |
| Skilled | 4,000 | 10,000 | 25 |
| Adept | 12,000 | 40,000 | 50 |
| Expert | 30,000 | 120,000 | 100 |
| Master | n/a | 300,000 | 150 |
| Grandmaster | n/a | 700,000 | 200 (Depths 9–10 dens) |

Master and Grandmaster need real effects (bonus +3/+4, lower miss chance, another Vitality step),
or they are only longer waits. Threat is capped at 200 today, so Grandmaster can only be taught by
the strongest dens. The exact numbers come from measured practice-per-round at each depth.

### 2. Levels give real power, on a steeper curve

Levels exist but do nothing. Proposal: 20 levels. Each level gives +2 max health plus half the CON
modifier. Every second level gives +1 to hit. The XP curve keeps doubling to level 10, then grows
by about 1.5 times per level, fitted so the ready level in the table above matches measured XP at
each target round. Kill XP already grows with depth, so shallow farming falls behind naturally.

### 3. Deep-tier gear from dens only

The shop stops at chainmail and the long sword. Plate and better come from Deep dens:

- Depths 4–6 keepers can drop tier-2 gear (plate, a heavier blade).
- Depths 7–9 keepers can drop tier-3 (masterwork).
- Depth 10 keepers can drop tier-4.
- Drops are a chance per keeper kill. Keepers return every 400 rounds, so a character farms its
  own frontier to gear up, which is the climb the target table needs.

This touches the save format: new variants or a quality field need `#[serde(default)]`.

### 4. Gold sinks that buy power

- **Smith:** upgrades a weapon or armor by +1 to +3, at escalating cost (for example 500, 2,000,
  and 8,000 gold). Each upgrade raises damage or protection by one.
- **Temple:** the healing fee scales with level instead of a flat 5 gold.
- **Cartographer:** sells clues that point to distant wilderness caches with deep-tier loot. This
  ties gold to exploration.

### 5. Wandering life and wandering encounters

- With lever 0 fixed, surface animals spawn again around the player.
- Danger and reward grow with distance from town: farther regions spawn stronger life and richer
  landmarks.
- Wandering encounters: groups that travel across regions instead of staying near home, on the
  surface and in the Deep. In the Deep, patrols arrive over time without a spawn point and scale
  with depth, so resting in the open and lingering under-powered cost something.

### 6. Retune Deep scaling for the lockout

Today hit bonus and armor rise by one per depth and health multiplies. Once levels, ranks, and
gear stretch out, steepen scaling from Depth 5 on, so a character two depths behind meets the
lockout gate above. Tune last, against the depth-curve benchmark.

## Order of work

Each step is measured with `deep_journey` and the depth-curve benchmark before the next.

1. Fix surface wildlife counting.
2. Levels and slower practice, together.
3. Deep-tier gear and the shop trim.
4. Gold sinks.
5. Wandering encounters and distance-scaled surface danger.
6. Deep scaling for the lockout.

The pilot needs updates alongside: buying upgrades, visiting the smith, staying on its frontier
depth until ready, and a 50,000-round cap.

## Open questions

- Names and effects for the ranks beyond Expert.
- How harsh death should be. Today death costs little, and harsher costs would also slow pace.
- Whether level-ups should offer choices or be automatic.
