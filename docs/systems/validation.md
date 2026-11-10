# Validation and playtest checklists

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Per-change correctness checklist

- [ ] Run the existing workspace tests and lint checks.
- [ ] Exercise the changed behavior through the real public core entry point.
- [ ] Exercise affected keyboard, targeting, and rendering behavior in the app.
- [ ] Cover intermediate and overlapping states, not only final outcomes.
- [ ] Preserve deterministic stream independence.
- [ ] Check save/load where the change affects durable state.
- [ ] Distinguish intentional rule changes from unexpected golden-fixture changes.
- [ ] Keep tests and default development behavior offline.

## Learning and commitment matrix

| Scenario | Required result |
| --- | --- |
| Ordinary relevant task for a beginner | Some useful learning |
| Identical safe task repeated indefinitely | Plateau, not unlimited mastery |
| Stealth objective without kills | Relevant learning and reward eligibility |
| Useful protection or healing | Credit based on actual eligible contribution |
| Self-harm/heal cycle | No unlimited progression source |
| Same encounter after reload | Previously spent opportunity remains spent |
| Death or a long camp | No time-only learning |
| Different meaningful action histories | Different supported developments |
| Major adaptation commitment | Explicitly confirmed, persisted, and constrained |
| Mature character changes equipment | Identity does not reset |
| New character | Independent potential and independent save |
| Skill disuse | No automatic loss |

## Synergy matrix

For authored fixtures, generated legal compositions, and newly discovered player combinations:

- [ ] Test each component alone.
- [ ] Test the intended combined effect.
- [ ] Test reversed action order.
- [ ] Test interruption between components.
- [ ] Test insufficient Focus or attention.
- [ ] Test repeated activation and effect refresh.
- [ ] Test actor death or target disappearance mid-sequence.
- [ ] Test duplicate trigger delivery.
- [ ] Test the maximum legal stack or number of targets.
- [ ] Confirm that logs and previews explain the result.
- [ ] Test a legal composition absent from the authored recipe examples.
- [ ] Test that unfamiliar component pairings earn appropriate integration evidence.
- [ ] Test canonical identity and save/load for player-composed techniques.

## Encounter lifecycle matrix

- [ ] Discover and immediately leave.
- [ ] Investigate without engaging.
- [ ] Enter through another valid approach.
- [ ] Complete optional beats out of the expected narrative order.
- [ ] Retreat during each apex phase.
- [ ] Save/load during preparation, resolution, and recovery.
- [ ] Die after partial objective progress.
- [ ] Claim a reward, reload, and revisit.
- [ ] Evict all relevant caches and revisit.
- [ ] Revisit after a large ecology time jump.
- [ ] Receive a late remote draft after fallback commitment.
- [ ] Receive the same accepted result twice.
- [ ] Resolve the site while a draft is still in flight.

## Death and recovery matrix

These check the proposed [death rule](actions.md#death-and-retreat), not the current composition proof.

- [ ] From the same injured, Focus-depleted state, compare dying with retreating: dying never leaves
      the character better off in Health, Focus, or accessible gear, including after equal recovery time.
- [ ] Death advances time and returns the character home with 1 Health and no Focus, not a refill.
- [ ] Home is safe; resting restores Health and Focus gradually as world time advances, and leaving
      before full recovery is allowed.
- [ ] All carried gear remains visibly recoverable at the death location, with ownership intact;
      gear stored at home is unchanged.
- [ ] Die again before recovering the first drop: both drops persist independently, including through
      save/load and cache eviction. Recovering either drop neither deletes nor duplicates items.
- [ ] Skills, committed development, and maximum Health are unchanged; elapsed time grants no practice.
- [ ] Wounded enemies recover during the absence, but durable encounter progress and claimed rewards
      persist. Death does not reset the world or deterministically promote the killer.

## Generated-content adversarial cases

- [ ] Unknown schema or rules version.
- [ ] Invalid JSON, oversized output, excessive nesting, or duplicate IDs.
- [ ] Cyclic prerequisites.
- [ ] Invented species, powers, placements, or facts.
- [ ] Impossible terrain access.
- [ ] Reward locked behind itself.
- [ ] Budget labels that understate actual mechanics.
- [ ] Prose contradicting the structured encounter.
- [ ] Instructions embedded in names or previous draft text.
- [ ] External URLs or code presented as required mechanics.
- [ ] Stale site revision.
- [ ] Timeout, refusal, cancellation, quota exhaustion, and network outage.
- [ ] Crash after dispatch but before a response is recorded.
- [ ] Duplicate delivery and retry after an ambiguous result.

## Playtest protocol

Run three kinds of test separately:

**Tactical test**

- Start with controlled fixtures and known identities.
- Observe decisions, not just win rates.
- Ask the player afterward what they predicted and why they changed tactics.
- Check whether the same encounter supports distinct approaches.

**Development test**

- Start from fresh potential.
- Do not tell the player which actions to grind.
- Watch whether ordinary play supports the intended emerging identity.
- Check whether commitments are understood and whether the player regrets hidden consequences.
- Compare the resulting characters, not only their inventories.

**Long-arc test**

- Follow real routes over multiple sessions.
- Track memorable rewards, retreats, ambitions, and returns.
- Include ordinary wildlife and exceptional sites.
- Check whether peon dominance and apex difficulty create contrast rather than boredom or whiplash.
- Check whether the player wants another character for a different identity.

For the first observed round, recruit at least three players who have not been coached through the design.
That is a small usability sample, not statistical proof of balance.

Record the seed, rules version, character history, loadout, encounter ID, and relevant action trace locally.
Do not transmit player data or recordings to a remote model without separate permission.

## Questions that determine whether the game is working

- Can the player name a creature difference that changed a decision?
- Can they explain one useful synergy without quoting a tooltip?
- Did a reward change their plans?
- Did a major commitment feel deliberate?
- Can they describe their character without naming a predefined class?
- Did ordinary enemies demonstrate growth?
- Did the apex feel extraordinary but understandable?
- Did retreat create an ambition rather than a sense of unfairness?
- Is another character appealing because its future would be different?
- Can the player describe that next character's intended weave, not just a new weapon skin?
- Are physical/physical and magical/magical combinations as interesting as physical/magical hybrids?
- Is the raging warded juggernaut powerful without invalidating subtlety, acrobatics, or field control?
- Does the game remain interesting with every remote service disabled?

## Performance evidence

Set exact budgets at CP0 or the checkpoint that introduces the workload, then measure:

| Workload | Required conditions |
| --- | --- |
| Startup | Fresh process, actual default entry point, representative saved world |
| Frontier action | First visit with uncached regions and life |
| Ordinary turn | Actual app action path, not only an isolated helper |
| Apex turn | Maximum legal actors, projectiles, reactions, and overlays |
| Save/load | Mature character and developed world, including partial encounter state |
| Catch-up | Largest supported elapsed-time interval and heaviest affected region set |
| Draft validation | Largest permitted payload, invalid as well as valid |
| Exploration memory | Long route through new regions with working-set eviction |

Report worst cases and distributions, not only averages. Separate cold and warm measurements. Include
build time only when the chosen budget includes it.

A release benchmark with precomputed regions does not establish frontier responsiveness. An accelerated
character fixture does not establish progression pacing. A model review score does not establish fun.
