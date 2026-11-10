# CP0–CP2: combat foundation

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## CP0. Freeze a testable first slice

**Goal:** turn the design into one bounded experiment rather than starting every system.

**Work**

- [ ] Select the first small technique catalog from the [technique catalog](techniques.md).
- [ ] Select the first foundation and adaptation subset.
- [ ] Map selected techniques to distinct physical and magical disciplines without adding duplicate progress bars.
- [ ] Specify the exact action timing, resource, and awareness rules for that subset.
- [ ] Choose a hand-authored encounter fixture built from a known seed and supported terrain.
- [ ] Define three test identities: close-range blaster, shuriken specialist, battlemage thief.
- [ ] Record which abilities are implemented and which fixture ambitions are deferred.
- [ ] Choose initial resource and fitting values explicitly as tuning data.
- [ ] Define measurable responsiveness budgets for the next checkpoint.

**Exit evidence**

- [ ] A player can understand each proposed build's distinct loop in plain language.
- [ ] Each selected technique has a standalone job and planned interactions.
- [ ] No selected mechanic depends on runtime remote generation.
- [ ] The scope fits one playable encounter before progression content expands.

**Stop or simplify if:** the first catalog already requires dozens of effects, a full crafting economy, or a
new simulation framework.

## CP1. Establish actions and interaction rules

**Goal:** a reliable foundation for techniques without breaking the current playable slice.

**Work**

- [ ] Add core-owned action query and execution.
- [ ] Preserve existing step, wait, blocked movement, and bump-attack behavior through the shared path.
- [ ] Add explicit cost and timing rules.
- [ ] Add the initial shared state vocabulary.
- [ ] Implement deterministic effect and reaction ordering.
- [ ] Add structured resolution events.
- [ ] Wire basic targeting and canceling into GPUI.

**Exit evidence**

- [ ] Existing tests pass or have documented intentional rule changes.
- [ ] Invalid targeting does not consume time or resources.
- [ ] Valid failed attempts consume the defined cost.
- [ ] Preview and execution agree for the same observable state.
- [ ] Reactions terminate under the strongest legal combination.
- [ ] Basic walking and peon combat remain fast to operate.
- [ ] The actual app is exercised, not only core unit tests.

**Stop or simplify if:** GPUI reimplements targeting rules, effects depend on log wording, or action ordering
cannot be explained without reading code.

## CP2. Prove skill synergy in one encounter

**Goal:** different combinations produce different decisions before adding a long progression curve.

**Work**

- [ ] Implement the selected starter techniques.
- [ ] Add Health, Focus, and sustained-capacity behavior for the fixture.
- [ ] Implement the minimum awareness and investigation model.
- [ ] Add trait-sensitive interaction rules using existing genes.
- [ ] Add deterministic developer loadouts for the three test identities.
- [ ] Supply a safe fixture recovery mechanism without prematurely removing all existing recovery.

**Exit evidence**

- [ ] Each identity solves at least one situation in a way another cannot reproduce by only changing a weapon.
- [ ] At least one hybrid integration is better than merely using its component actions independently.
- [ ] A creature trait changes the useful approach.
- [ ] No identity is forced to kill repeatedly to demonstrate its main play loop.
- [ ] Zero Focus leaves a usable baseline action.
- [ ] No action/refund/healing loop creates unlimited resources.
- [ ] A player can explain why the synergy worked.
- [ ] The selected tracks differ in governing rules, not just damage type or animation.
- [ ] A compatible composition absent from the authored examples executes through the same rules.

**Playtest checkpoint:** short observed sessions focused on one encounter. These tests establish tactical
possibility, not the quality of use-based character development.

**Stop or simplify if:** every build converges on the same action sequence or the optimal strategy is always
spamming the highest damage action.
