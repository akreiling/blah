# Retreat and reliable recovery

[Actions and recovery](actions.md#death-and-retreat) · [Skill pacing](learning.md#stage-3-measured-prototype-pacing)

## Reproduce

```sh
just skill-pacing 16 1200 compare
just test
just lint
```

The probe starts fresh `Game::new` characters, without injected practice, creatures, resources, or
positions. `baseline`, `cautious`, and `compare` select the policy; omitted means baseline. Both policies
use the same six loadouts and seeds. Policies see only the visible neighborhood, never hidden creatures
or their banked action energy. The offline escape scorer uses full state to avoid mistaking an
out-of-view, long-sensed hunter for a successful escape.

## Policy versus rules

The baseline keeps its original behavior: seek creatures, retreat at 10 Health or specialist Focus
below 8, but attack adjacent creatures before moving away. Both policies now query `Game::can_rest`
instead of assuming the preparation safety rule also controls recovery.

The cautious policy:

- Starts returning at two-thirds Health or specialist Focus below 14.
- Estimates guarded fights using minimum strike previews, two extra miss turns, maximum incoming
  damage rolls, and rounded-up creature actions. It reserves 10 Health and sums visible nearby threats.
  This is a pessimistic policy estimate, not a combat prediction or a new game rule.
- Avoids fights it cannot afford, and prioritizes movement over adjacency attacks while returning.
- Chooses unoccupied steps by immediate risk, sensing exposure, then its route home. Outside retreat,
  it accepts exposure to fights within its estimated budget.
- Does not spend a turn toggling sneak during pursuit unless empty Focus prevents movement.
- Fights only when cornered or when the selected encounter fits its budget.

Early exploratory avoidance was almost non-combat: zero deaths in 48 short runs, but only 74 kills,
and 29 unfinished returns. That is not evidence of a usable progression route. The final policy also
allows approaches to affordable fights; the results below use that policy.

## Recovery fix

Previously, any creature in view blocked home rest. The baseline could leave home to clear them,
die, wake weak, and repeat. The original 96-run skill sample recorded 516 deaths. A regression starting
at home with 1 Health, zero Focus, and nearby threats reproduced the blocked recovery.

Home rest now shelters the wanderer for that action and restores the same 2 Health and 3 Focus.
Creatures still observe, bank and spend speed, move, and spawn. They are not removed or healed by rest.
Other actions remain exposed, even at home. Fully recovered characters cannot take a protected rest
turn. Death still costs 1,000 turns, leaves 1 Health and zero Focus, and grants no practice.

This is a recovery floor for living retreats and deaths alike, not a general invulnerability flag.
It does not establish that fighting beside home is balanced; home-based attack/rest tactics and
persistent encounter recovery need separate testing.

## CP5 supplies

The first-journey slice adds exposed `P` camping for 1 Health and 2 Focus when no creature is in view, and
`V` salves for 10 Health during any normal turn. Fresh characters carry two salves; caches add one up to
the two-salve cap, and an accepted home rest refills them. Camping does not consume a supply. The home rest
remains sheltered from attacks, unlike camping and salves.

Passive Health regeneration is gone: Health returns only through home rest, camping, and salves. Death still
wakes the wanderer at home with 1 Health, and sheltered home rest brings it back.

## Episode accounting

The third CSV table reports each run's episode counts and upper-median action times:

- A **retreat** starts when the policy enters its returning state. It may begin before injury or
  detection. It ends fully recovered at home, in death, or unfinished at the action cap.
- An **escape** requires a previously observed pursuer still alive in the world and no creature
  currently able to pursue. Merely losing view, killing every pursuer, and death-driven clearing do
  not count. Leaving a territorial creature's defended ground ends its pursuit. Escape is a milestone,
  not the end of recovery; it can be followed by re-engagement or death.
- **Recovered** means both resources are full at home, including returns that needed no healing.
  The rest-action count distinguishes those from actual recovery. Arriving alive with depleted
  resources is not completion.
- Each death starts a separate **wake recovery**, not another successful retreat. A second death
  before full recovery fails that wake episode. Wake and living-retreat timings stay separate.
- Unfinished episodes are explicit. Each class satisfies `started = recovered + deaths + unfinished`.
  Arrival medians include living returns that reached home; recovery medians include only completions.
  Death's time jump is included in total game turns, not in command counts.

## Final matched-seed results

Seeds 0–15, 1,200 actions per run, six loadouts, **96 runs and 115,200 actions per policy**:

| Measure | Baseline | Cautious |
| --- | ---: | ---: |
| Deaths | 95 | 77 |
| Death-free runs | 45/96 | 60/96 |
| Encounters | 6,304 | 2,711 |
| Kills | 5,743 | 705 |
| Retreats started | 1,349 | 1,004 |
| Escapes during retreats | 142 | 412 |
| Returned home fully recovered | 1,229 | 850 |
| Retreats ending in death | 95 | 77 |
| Unfinished retreats | 25 | 77 |
| Wake recoveries completed | 94/95 | 76/77 |
| Deaths during wake recovery | 0 | 0 |
| Unfinished wake recoveries | 1 | 1 |
| Home recovery blocked | 0 | 0 |
| Runs with weapon rank above novice | 95/96 | 28/96 |

Per-run median living-recovery times range from **16–33 actions** for baseline and **1–288**
for cautious play. Per-run median wake recovery is **13–14 rest actions** for both.
These are ranges of per-run medians, not pooled episode quantiles. Long and unfinished returns remain
important even when the character survives.

The comparison shows that retreat is feasible, but caution trades away substantial combat and weapon
growth. It does not show universally escapable encounters, an optimal cautious player, or final balance.
The fall from the historical 516 deaths to 95 baseline deaths includes the home rule and the caller's
updated recovery query; it is not a pure comparison of decision policies.

## Regression coverage

- Home recovery starting weak after a living return and after death, with nearby lethal fast threats.
- Creature movement during sheltered rest, gradual recovery, no skill credit, and encounters preserved.
- Ordinary waiting and attacking remain lethal; fully recovered rest is rejected.
- Cautious movement instead of attacking a dangerous adjacent target.
- Hidden-but-tracking observers and killed pursuers are not scored as escapes.
- Separate death and wake episodes, conservation of episode outcomes, deterministic replay of both
  policies, and property-generated cautious runs across seeds, loadouts, and action caps.
