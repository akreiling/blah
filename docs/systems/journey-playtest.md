# CP5 whole-journey pass

[CP5](checkpoints-development.md#cp5-complete-one-worthwhile-journey) · [Recovery](retreat-recovery.md)

## Reproduce

```sh
mise exec -- just journey-playtest 64 500
mise exec -- just journey-playtest 24 500 0 blind
mise exec -- cargo test -p blah-core --example journey_playtest
```

The deterministic pilot starts with a fresh `Game::new(seed)` and the ordinary starting kit. It uses only
public actions and information: observed terrain, visible creatures and sites, lookout discoveries, and,
unless `blind` is selected, the same field-sign destinations shown in the app. It does not teleport, grant
practice or supplies, remove creatures, attack, or read hidden guard/reward state. Paths use remembered
terrain and avoid occupied tiles; unknown routes still require exploration.

The pilot gathers useful herbs, brews when the view is clear, sneaks while watched, uses supplies, and
retreats for low Health or after earning a cache weapon/relic. It recovers at home, trades an unsuitable
weapon if there is room, then sets out again. **Complete** means it returned with a reward, used traded
supplies or an actual relic effect on a later outing, and claimed another cache, without dying. Merely
owning a relic does not count as using it.

The command prints every seed, including failures, with action, movement, camp, rest, gathering, brewing,
claim, relic, trade, draught, and death counts. Milestones record decisions. Saves round-trip through JSON
at reward and home milestones; the regression tests also compare complete routes with a reload before
every action.

## Problems found and changes

- **Home management stayed blocked after recovery.** Followers remained visible, so the fully recovered
  wanderer still could not prepare, attune, or trade. Home now shelters these management actions. A trade
  passes one protected turn; ordinary waiting, movement, and combat remain exposed. Creatures are not
  removed, healed, or killed. Full characters still cannot buy free protected rest turns.
- **Blind travel often missed the region's useful sites.** The FIELD SIGNS panel now gives directions to
  the region's possible cache site and ready patches of missing herbs. Signs reveal no route terrain,
  loot, or guards, do not instantiate encounters, and are regenerated rather than saved. The player still
  has to scout and judge the approach. Placement, guard strength, herb recipes, and regeneration rates
  were not changed.

## Measurements, 2026-11-10

The final pilot was run on seeds 0–23 with a 500-action cap in three configurations. For the first row,
the pre-pass home preparation/trade rules were temporarily restored and field signs disabled. The other
two use the shipped home rules; `blind` changes the pilot's information, not the world.

| Configuration | Seeds claiming a cache | Complete | Blocked home management | Deaths |
| --- | ---: | ---: | ---: | ---: |
| Original home rules, blind | 10/24 | 3/24 | 8/24 | 2 |
| Sheltered home management, blind | 10/24 | 4/24 | 0/24 | 3 |
| Sheltered home management, field signs | 20/24 | 10/24 | 0/24 | 2 |

The final default run over seeds 0–63 recorded:

| Outcome | Seeds |
| --- | ---: |
| Complete | 27 |
| Action cap reached | 14 |
| No route avoiding visible creatures | 8 |
| Recovered after abandoning the outing | 7 |
| Died | 8 |

52/64 seeds claimed at least one cache. The sample brewed 15 draughts in the field and acquired relics on
three seeds. Guidance made early cache rewards easier to find, not every non-combat journey safe.
The guided pilot brewed fewer draughts than the blind pilot in the 24-seed comparison (4 versus 10):
it found cache rewards sooner and used home trade-ins instead. This is a supply-path trade-off, not proof
that herb access improved.

## Repeatable route evidence

- **Seed 10:** gathers four times, brews twice in the field, recovers at home, trades a reward, drinks on
  the next outing, and claims a second cache. Completes in 181 actions without a fight or death.
- **Seed 16:** uses a wild garden, brews twice, recovers and trades at home, drinks carried supplies, makes
  a low-Health retreat, recovers again, then claims the next cache. Completes in 137 actions.
- **Seed 18:** returns and trades, finds a shrine on the next outing, acquires Hushstone, and uses its
  entering-sneak Focus saving four times before the next cache. Completes in 250 actions.
- **Seed 2, blind:** reproduces the follower-at-home case with real travel. The old rule stopped at home;
  the sheltered management rule allows preparation, a trade, and another outing.

Every-action reload comparisons cover seeds 10 and 18. Existing tests still cover deaths, guard
despawn/reentry, one-time claims, and depleted-resource persistence.

The GUI smoke check verifies that moving updates cache/herb directions and that the workshop can prepare
a staff at home beside a live sentinel, without advancing the turn or removing the creature. Outside-home
preparation rejection and protected trading are covered by regression tests.

## Limits

This is an automated non-combat policy, not an independent human playtest or a completion-rate target.
`no-route` means the policy could not find a route avoiding currently visible creatures; it does not prove
the world is disconnected. A budget result may include conservative detours or backtracking. Some routes
remain dangerous, long, or supply-poor. The sample does not establish the value of every relic, weapon
identity, or continued combat build, and it does not pass CP6's authored-arc gate.
