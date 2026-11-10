# Practice, learning, and specialization

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Implemented prototype: weapon practice

Stage 1 of skill growth is in [`skills.rs`](../../crates/core/src/skills.rs). Stage 2 adds Focus and
stealth, and stage 4 adds first-rank move unlocks. Stage 3 measured pacing and tuned stealth evidence.
Moves are implemented; saving is not.

- **What trains:** physical damage from weapon delivery (a free strike or a weapon technique) that actually
  lands. Misses, waiting, moving, resting, sneaking, dying, refitting, and elapsed turns teach nothing.
  Touch and ground-area magic do not train weapons.
- **Per weapon family:** dagger, spear, club, and staff each grow separately. Practice belongs to the
  character, not the item. Equipping another weapon neither transfers nor resets it.
- **Threat scales learning:** threat is roughly the percent of your full Health a creature would take
  while an unskilled fighter with that weapon beat it down: its attack against your defense, its full Health
  against your weapon's base damage, and its speed. Injury, Focus, and earned bonuses do not change it.
  Skittish and inert (speed 0) creatures have zero threat.
- **Threat caps learning:** below 10 teaches nothing. Below 25 teaches up to *practiced*, below 50 *skilled*,
  below 100 *adept*, and above that *expert*. Weak creatures stop teaching once you outgrow them. A
  lower ceiling never removes practice you already earned.
- **Finite per creature:** each individual can pay out practice for at most its full Health in damage.
  Overkill does not count, and magic damage uses up the same allowance. Healing a creature, including the
  recovery after your death, does not refill it.
- **Effects:** ranks are words only (novice, practiced, skilled, adept, expert). They add +1 weapon damage
  at practiced and skilled, +2 at adept and expert, and lower the weapon miss chance from 10% to 9%, 8%, 7%,
  and 6%. Previews use the same numbers. Bursts, touch, and area damage are unchanged.
- **Visibility:** the panel shows the equipped weapon's rank. A log line announces each rank gained.

Practice thresholds are 1,000, 4,000, 12,000, and 30,000; one landed damage point earns its threat in
practice. These remain prototype numbers. The 40-Health trial sentinel has a dagger threat of 133, so
one full dagger fight against it goes from novice to skilled. Ordinary wild creatures teach far less.
Skills are not saved; there is no save system yet.

## Implemented prototype: Focus and stealth practice

Stage 2 reuses the same engine: ranks, teaching ceilings set by threat, and finite lessons per creature.
Threat is the same number as for weapons, measured with the equipped weapon.

**Focus** learns from Focus actually paid for a technique that did real damage to a creature. Evidence is
the Focus cost times the creature's threat. A miss, a free strike, sneaking, and an empty area cast teach
nothing. An area cast counts its most threatening creature, and only if the cast used up some of that
creature's damage allowance. Since every credited technique uses up some of that allowance, a creature
can teach Focus only a limited number of times. Each rank adds 2 to the Focus pool, from 24 up to 32.
Growth raises the cap, not current Focus; resting at home fills the larger pool.

**Stealth** learns three ways. Each creature has 12 lessons to give, shared across all three:

| Source | Lessons | What qualifies |
| --- | --- | --- |
| Unseen sneaking move | 1 | A paid or free sneaking move ends inside a creature's normal sensing range, and it still has not found you |
| Landed opening strike | 3 | A hit that actually exploited an unguarded opening |
| Escape | 3 | A creature that was tracking you at the last turn's observation loses you |

Each lesson is worth 2 times the creature's threat. Entering sneak, standing still, sneaking with no
threat near, and creatures below threat 10 teach nothing and use up no lessons. Being revealed by your own
attack and then leaving range is not an escape, and death clears tracking without paying for it.

Stealth effects, by rank (novice, practiced, skilled, adept, expert):

- Sneaking takes 6, 7, 7, 8, 8 tiles off an unalerted creature's initial sensing range.
- Sneaking moves: novices pay for every move; one move in every 4, 3, 2, 2 is free.
- Landed opening strikes gain 0, +1, +1, +2, +2 physical damage, on top of weapon practice.

The panel shows the Focus and stealth ranks beside the weapon rank, and the log announces each rank
gained. In the dagger trial, one opening strike on the threat-133 sentinel earns 798 stealth practice,
below *practiced*. Two paid technique hits still take Focus to *practiced*.

## Implemented prototype: practiced-rank moves

The first move for a skill unlocks automatically when its character-owned rank reaches *practiced*.
There is no separate unlock ledger. Weapon moves also require that weapon family to be equipped, but
earning a move survives refitting and death. The prepared technique is never replaced, and ordinary
bump strikes remain free. Moves and skills are not saved across application restarts.

| Skill | Move | Focus | Effect |
| --- | --- | ---: | --- |
| Dagger | **Withdraw** | 3 | Strike an adjacent target, then step one tile directly away, even on a miss. Requires an empty, passable retreat tile. |
| Spear | **Reach** | 2 | Strike a target exactly two tiles away through one empty, passable intervening tile. |
| Club | **Shove** | 3 | Strike an adjacent target; on a hit, push a survivor one tile directly away if that creature can occupy the tile. The strike still works when there is no room to push. |
| Staff | **Sweep** | 4 | Strike every cardinally adjacent target once. Diagonals are not included; Focus is paid once for the whole sweep. |
| Stealth | **Slip away** | 4 | Move two empty, passable tiles in one direction in one turn and enter sneaking. The ordinary final observation decides whether tracking is broken. |
| Focus | **Pulse** | 0 | Make a weak adjacent magic attack for 1 damage on a hit. It can be used at zero Focus and trains neither weapon nor Focus. A hit that spends the target's shared finite damage-learning allowance draws 2 Focus back, up to capacity; that allowance never refills, so a creature cannot be farmed. |

Every accepted move takes one turn. Its Focus cost is paid once, including on a miss; Pulse costs
nothing. Invalid range, blocked movement, insufficient Focus, or a locked move spends neither Focus
nor a turn. Weapon moves use the ordinary trained basic strike, not the prepared aim or payload.
Previews and logs identify valid targets, damage bounds, movement, optional pushes, Focus cost, and
any Focus a hit would draw.

In the app, **Tab** cycles the Weapon, Stealth, and Focus slots. **Alt + direction** uses the selected
move; the Weapon slot follows the currently equipped family. The status panel shows its lock or a
current preview. See [the move controls](composition-poc.md#try-it) for the full key list.

## Stage 3: measured prototype pacing

Run `just skill-pacing [seeds] [actions] [baseline|cautious|compare|move-compare]`
(defaults: 8 seeds, 600 actions, baseline). It creates fresh
`Game::new` characters and drives the same public actions as the app. No injected skill, resources,
or trial creatures. `just test` includes the probe's determinism, action-accounting, and play-style checks.

The six policies use all four weapons as free-striking fighters, a sneaking dagger opener, and a
staff-equipped touch-fire caster. They seek visible non-skittish creatures, explore cardinal waypoints
six tiles from home, and try to retreat at 10 Health or, for specialists, below 8 Focus. They rest
at home. The caster falls back to free staff strikes;
the sneak uses the paid technique only at an opening. These are simple, fallible policies, not expert
players. They do not assess whether a particular creature is safe to fight.

Output has three CSV tables: per-run totals, rank milestones, and retreat/recovery episodes.
The optional cautious policy and episode definitions are in [retreat and recovery](retreat-recovery.md).
An **action** is a command that spends
a turn, including rest and sneak toggles. **Turns** additionally include death's 1,000-turn penalty.
An **encounter** counts a distinct non-skittish creature observed in its normal sensing range or
targeted by an attack, including unseen observers. It is not a completed fight, kill, or guarantee of
teaching eligibility. Milestones report upper medians among runs that reached the rank, alongside
the reached/run count and minimum/maximum action count. Unreached ranks remain explicit, not zero.
Encounter medians are calculated separately, not necessarily from the median-action run.

Historical first comparison, at commit `d66a93a`, before sheltered home rest:
seeds 0–15, 1,200 actions each, 96 runs per version (115,200 actions).
That baseline cleared nearby creatures before resting; current home recovery no longer requires that.
The provisional target was to bring first-rank stealth into the same rough range as early weapon
experimentation, without moving all skill thresholds to compensate for an artificial trial.

| Policy / skill reaching practiced | Before: median actions | After: median actions | After: range | After: reached | After: median encounters |
| --- | ---: | ---: | --- | --- | ---: |
| Dagger / weapon | 80 | 80 | 14–192 | 16/16 | 4 |
| Spear / weapon | 87 | 87 | 24–196 | 16/16 | 7 |
| Club / weapon | 132 | 132 | 25–383 | 16/16 | 9 |
| Staff / weapon | 71 | 71 | 14–170 | 16/16 | 5 |
| Sneak / stealth | 30 | 107 | 12–699 | 16/16 | 6 |
| Caster / Focus | 183 | 183 | 23–481 | 16/16 | 13 |

**Changed:** stealth evidence per lesson from 5 × threat to 2 × threat. The same finite 12-lesson
allowance and qualifying outcomes remain. Sneak-policy stealth reached skilled in 9/16 runs
(median 517 actions among those nine), adept in 1/16, and expert in 0/16, versus 16/16, 7/16,
and 1/16 before. Its fastest practiced result is still 12 actions, so this is not a minimum-time gate.

**Unchanged:** weapon and Focus evidence, rank thresholds, threat ceilings, and skill effects. Wild
weapon and caster Focus progression does not reproduce the trial's one-fight/two-hit pacing.
Slowing every skill based on that fixture would address the wrong input.

**Limits:** the tuned policies died 516 times in aggregate (519 before). Successful retreat is not
assured. Death loses time and resources and awards no practice; previously earned practice persists.
Changing learned stealth also changes later actions through its concealment and resource effects.
These probes support a small relative pacing correction, not a claim of balanced human play, a
death-free progression route, or a final mastery curve. Broader exploration, safe evasion routes,
area casting, and human playtesting remain unmeasured.

## Stage 4: first learned-move balance probe

Run `just skill-pacing 16 1200 move-compare`. This compares the existing baseline and cautious pilots
with matching move-aware variants on the same 16 seeds and six styles, for 96 runs per policy. Every
run starts from `Game::new`; ranks, Focus, and move unlocks are earned normally. The move-aware pilot
uses weapon moves when they have a valid opportunity, tries Reach before a one-tile advance, uses
Shove only when a push is possible, sweeps only when at least two targets are adjacent, slips away
while retreating when its route is valid and no observed pursuer gets closer, and has its caster use
Pulse only when the Focus it draws would afford the next prepared technique. This is a simple policy
probe, not an optimal player model.

The output adds final Health/Focus per run and a move table: runs unlocking the skill, median unlock
action, affordable opportunities, uses, hit actions, actual damage, Focus spent, Focus drawn, movement steps, and
successful Shove pushes. An opportunity means at least one direction passed the real move preview.
The paired policies share seeds and initial styles, but their later worlds diverge because their
actions differ.

| Policy pair | Total deaths, no moves → moves | Death-free runs, no moves → moves | Mean kills/run, no moves → moves | Mean Focus left, no moves → moves |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 95 → 74 | 45/96 → 57/96 | 59.82 → 59.79 | 20.69 → 12.24 |
| Cautious | 77 → 72 | 60/96 → 62/96 | 7.34 → 6.53 | 22.43 → 4.89 |

Pulse was first a free 1-damage strike with no other effect. A caster that used it whenever it could
not afford its technique replaced nearly every staff strike: all 16 such runs ended with a novice
staff (versus one in 16 without moves), kills fell by 8.5 per caster run, and deaths rose from 8 to 84.
Pulse now draws 2 Focus on a hit. That alone was not enough while it still displaced every strike
(62 deaths). Used only to top up the next cast, it was chosen 151 times, drew 280 Focus, and gave
11.7 more casts per caster run, with kills unchanged (46.56 → 46.50) and deaths 8 → 10. A 1-Focus
draw almost never completed a cast (12 uses); a 3-Focus draw added no more casts and had 12 deaths.
Constant use of Pulse remains a poor choice because it starves weapon practice.

The other moves had different results. In the baseline-move runs, spear Reach was used 499 times
(2,612 actual damage for 998 Focus), club Shove 601 times (307 pushes), staff Sweep 50 times across
101 targets, and sneaking Slip away 262 times for 524 steps. In the 16 spear runs, Reach coincided
with 5.06 more mean kills per run and nine fewer total deaths; in the staff runs, Sweep coincided
with 1.81 more mean kills per run and 13 fewer total deaths. Shove and Withdraw had smaller or mixed
survival and kill changes. These are policy-level associations, not isolated causal estimates.

**Decision:** retain the current Focus costs (Withdraw 3, Reach 2, Shove 3, Sweep 4, Slip away 4,
Pulse 0) and give Pulse a 2-Focus draw on a hit. The small sample supports Reach and Sweep as useful
choices, but does not justify a broad cost change; the aggressive and cautious policies also trade
different amounts of Focus for different outcomes. Pulse pays off as a top-up for the next cast, not
as a replacement for the free strike. The cautious caster rarely earns it (2 of 16 runs), so its
effect there is unmeasured. These numbers are provisional, and the probe does not measure human use
or late-rank moves.

## Award learning from outcomes

The core emits a resolved activity record after an action or encounter outcome. Learning reads that record;
it does not infer behavior from UI commands, text logs, or wall-clock time.

Relevant facts include:

- The actor and stable activity or encounter identity.
- The technique used and the part of it that actually took effect.
- What obstacle, threat, or resource was involved.
- The actor's state, including active protection and awareness relationships.
- The result: effective damage, successful concealment, useful healing, completed extraction, interruption,
  or another defined outcome.
- Which compatible interaction genuinely occurred.

Do not store every possible fact forever. Each fact needs a rule that consumes it. Aggregate credited
evidence into bounded records and persist enough identity to prevent repeat claims.

## Meaningful practice

Meaningful does not mean "nearly died." It means the action accomplished or attempted a relevant task at a
level that can still teach this character something.

Examples:

| Activity | Qualifying evidence | Non-qualifying loop |
| --- | --- | --- |
| Swordplay | Controlling a real opponent, exploiting a valid opening | Repeated attacks against an inert invulnerable target |
| Protection | Preventing an independently caused threat | Manufacturing harmless impacts solely to trigger credit |
| Stealth | Passing an aware-capable observer or completing a concealed objective | Toggling sneak on an empty tile |
| Healing | Restoring meaningful injury during or after a real situation | Self-injury followed by infinite healing |
| Gathering | Successfully extracting an available resource with relevant difficulty | Repeatedly interacting with a depleted node |
| Ranged control | Landing or redirecting a meaningful projectile action | Firing into empty ground forever |
| Integration | Two abilities contribute to the same actual outcome | Activating a useless ward before every unrelated action |

Failure can provide limited relevant learning when it genuinely tests a skill. It must not become better
than accomplishing the task, nor encourage deliberately sabotaging encounters.

## Learning credit model

Prototype pipeline:

1. Validate that the activity can teach this character at its current stage.
2. Identify the finite learning opportunity associated with the situation.
3. Compute useful contribution from the resolved effect, not requested magnitude.
4. Apply relevance to the practiced foundations and proficiency.
5. Allocate a bounded portion to an actual integration, if one occurred.
6. Record the claim under the durable opportunity identity.
7. Update visible development evidence.

One action may teach several connected things, but it does not multiply the total opportunity simply because
it carries many tags. Teaching ten nominal disciplines with one action must not generate ten times the
advancement.

Repeated attempts in the same situation share a learning allowance. Leaving sight, reloading, changing
weapons, or letting a creature respawn must not mint a new allowance for the same unresolved encounter.

## Baseline competence and advanced learning

Practice has two broad regimes:

- **Basic competence:** ordinary relevant activity can teach it; experimentation should not require rare
  treasure or difficult combat.
- **Deep development:** new situations, substantial execution demands, and related discoveries are needed.
  Solved harmless tasks plateau.

This allows local experimentation while preserving a long arc of adventuring. It also avoids tying all
advancement to kills or arbitrary creature "levels."

## Integration learning

Integration is not awarded simply because two skills were used during the same expedition. The relationship
must matter.

Examples:

- A ward absorbs a blow while a committed close-range cast completes.
- A decoy causes an observer to investigate elsewhere, enabling a concealed crossing.
- A shove creates enough distance for a ranged action that otherwise could not have been completed.
- A movement technique preserves concealment while a quiet spell is prepared.

Start with explicitly authored relationship predicates over resolved effect provenance, such as an actual
carrier delivering a payload or a ward preventing an interruption. These apply to compatible components
generically, not only listed technique pairs. Do not build a universal inference engine that invents causal
relationships from arbitrary action histories.

Each relationship can support several builds and novel compositions. Avoid one secret prerequisite recipe
for every named hybrid. An unforeseen legal interaction must still receive appropriate practice evidence.

## Developmental commitments

Learning evidence makes relevant adaptations available. The player chooses among supported developments,
with structural costs shown before commitment.

The player cannot simply save up a generic currency and buy an unrelated mature identity. Learning a new
direction means doing it, and the character may no longer have the structure to master it deeply.

Progress remains attached to the character, not the currently equipped item. An enchanted weapon can assist
performance but does not become an experience multiplier or reusable training loophole.

## Time and repetition

Three clocks must stay distinct:

| Clock | Used for | Must not do |
| --- | --- | --- |
| Action and game time | Actions, recovery, ecology, death consequences | Grant mastery solely because turns elapsed |
| Meaningful experience | Learning and development | Count identical safe repetition without limits |
| Real-world time | External service budgets and operational scheduling | Train the character, advance the world while the application is closed, or gate ordinary play |

A camping interval and a death time jump can advance ecology but do not award practice.

## Progression pacing

The intended shape is:

- Early useful experimentation.
- A noticeable but explained transition into commitment.
- Many intermediate improvements within the chosen identity.
- A very long path toward extraordinary mastery.

Do not set the final progression curve by guessing a total number of hours. First measure how much relevant
experience normal play produces, including non-combat play, failure, retreat, and difficult exploration.

Later checkpoints must establish a pacing target and test full routes from fresh characters. Accelerated
developer characters can test endgame mechanics but cannot validate the journey to them.
