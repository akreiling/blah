# CP3–CP7: development and a complete game loop

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## CP3. Make characters grow into those identities

**Goal:** replace developer-assigned identity with earned, visible development.

**Work**

- [ ] Emit and consume meaningful practice evidence.
- [ ] Add bounded learning opportunities.
- [ ] Implement foundations and proficiencies for the selected activities.
- [ ] Implement explicit adaptation commitments and finite structural capacity.
- [ ] Add the first integration developments.
- [ ] Add development previews, deferral, and confirmation.
- [ ] Make related branching possible without allowing cheap distant reinvention.

**Exit evidence**

- [ ] Fresh characters following different real action histories develop differently.
- [ ] Basic experimentation does not silently force an irreversible path.
- [ ] Major commitments are explained before acceptance.
- [ ] A mature fixture character cannot acquire every conflicting high-tier adaptation.
- [ ] Technique swapping cannot change foundations.
- [ ] Repeated safe loops plateau.
- [ ] Stealth, protection, healing, and gathering can earn relevant evidence where implemented.
- [ ] Time passing, camping, and death award no mastery.
- [ ] Skills do not decay through disuse.

**Playtest checkpoint:** observe fresh starts, not just accelerate characters to their final adaptations.

**Stop or simplify if:** players grind contrived actions to become the intended identity, or the best strategy
is to postpone all commitment indefinitely for a universal advantage.

## CP4. Preserve the character and world

**Goal:** a session can end without erasing the meaning of development.

**Work**

- [ ] Add the versioned SQLite save adapter.
- [ ] Save character development, resources, inventory placeholders, and practice claims.
- [ ] Save current actor and effect state required by implemented mechanics.
- [ ] Implement atomic action-boundary snapshots.
- [ ] Record terrain/rules versions and reject unsupported saves safely.
- [ ] Add a supported new-character path with a separate world save.
- [ ] Keep caches reconstructible without changing authoritative state.

**Exit evidence**

- [ ] Continuous play equals save/load/resume for scripted action sequences.
- [ ] Reloading does not repeat learning or undo a committed adaptation.
- [ ] Pending actions and sustained effects resume consistently.
- [ ] A failed write leaves the last committed save usable.
- [ ] Two characters cannot overwrite each other's progress.
- [ ] Missing or incompatible saves fail clearly without destructive cleanup.

**Stop or simplify if:** persistence serializes incidental caches as authority or saves depend on current
biome-table indices remaining unchanged forever.

## CP5. Complete one worthwhile journey

**Goal:** exploration, preparation, treasure, and recovery support the growing character.

**Work**

- [ ] Add a small number of durable landmark templates.
- [ ] Add ordinary equipment, a few specialist items, and a small relic set.
- [ ] Add inventory ownership, loadouts, and explicit switching rules.
- [ ] Add a few gathering resources and useful consumables.
- [ ] Add safe camping, home recovery, and emergency healing.
- [ ] Replace passive recovery only after the new recovery path is complete.
- [ ] Add durable treasure claims and resource depletion.

**Exit evidence**

- [ ] A complete journey yields something that changes a later decision.
- [ ] The player can choose to continue or recover for understandable reasons.
- [ ] A non-combat approach reaches at least one meaningful reward.
- [ ] Supplies and ammunition have no circular dependency that strands a basic character.
- [ ] Items support identity without bypassing major structural commitments.
- [ ] Save/load, death, and cache eviction do not duplicate treasure.
- [ ] Home is useful without every minor discovery requiring a return.

**Playtest checkpoint:** run the whole journey, including preparation and recovery. A compelling fight alone
does not pass this gate.

## CP6. Build an authored arc and a formidable apex

**Goal:** prove the dominance-to-apex contrast before remote drafting.

**Work**

- [ ] Implement an encounter definition and runtime format.
- [ ] Build one authored arc with attraction, warning, escalation, apex, and aftermath.
- [ ] Add durable exceptional actors separate from ambient despawn.
- [ ] Implement the apex's actual capability cycle and telegraphs.
- [ ] Add retreat, abandonment, and recovery behavior.
- [ ] Provide multiple mechanically supported opportunities.
- [ ] Add replay fixtures for each phase and reward transition.

**Exit evidence**

- [ ] An advanced character clears ordinary inhabitants convincingly.
- [ ] The apex still demands a plan.
- [ ] Its power is related to the earlier clues and lesser encounters.
- [ ] Three test identities have distinct opportunities against the same immutable encounter.
- [ ] A weak character receives warning before catastrophic commitment.
- [ ] The player can return stronger without the site secretly scaling upward.
- [ ] Death and reload preserve the arc and its claims.
- [ ] Late phases remain readable at actual viewport size.

**Stop or simplify if:** difficulty comes only from inflated Health, every build needs the same item, or
unavoidable attacks are obscured by presentation.

## CP7. Prove committed progression and power contrast

**Goal:** test that the system creates enduring identities and a satisfying long-term direction.

**Work**

- [ ] Expand only the technique branches needed to give the test identities depth.
- [ ] Add intermediate and advanced refinements with meaningful trade-offs.
- [ ] Add several encounter contexts, including unfavorable but interpretable ones.
- [ ] Establish pacing targets from observed ordinary play.
- [ ] Add high-power developer fixtures to test scale and constraints.
- [ ] Keep fresh-start progression tests separate from accelerated endgame tests.
- [ ] Add the weave-proof fixtures for poison-stealth magic, the raging warded juggernaut, and area frost.
- [ ] Include an acrobatic physical integration and a magical/magical integration.
- [ ] Use the relevant Jev question-bank cases to identify concrete hypotheses for tests.
- [ ] Build a bounded player composition workspace using known components and generic compatibility rules.
- [ ] Show core-owned legality, timing, cost, and structural-requirement previews with actionable rejection reasons.
- [ ] Save canonical player-composed structures and support preparing, practicing, and refining them.

**Exit evidence**

- [ ] New characters genuinely explore different development.
- [ ] Advanced identities do not converge on a universal adaptation set.
- [ ] Old threats become meaningfully easier without being deleted from the world.
- [ ] Advanced foes challenge specialized power through capabilities and context.
- [ ] Refinement adds control or scope, not only tiny percentages.
- [ ] Adjacent development is possible; radical reinvention is not a cheap menu operation.
- [ ] No tested technique or relic is mandatory across every intended build.
- [ ] Progress does not depend on repetitive solved tasks.
- [ ] Each requested identity has a distinctive activity sequence and a credible path to mature power.
- [ ] Different builds using the same disciplines still express different commitments.
- [ ] Players can name a specific next character they want to try without rejecting the current one as a mistake.
- [ ] An independent playtester can develop an effective build outside the named fixture identities.
- [ ] Structured player composition validates contracts rather than checking a finished-recipe whitelist.
- [ ] An uncoached player can propose, understand, save, and use an unfamiliar legal composition in the actual app.

**Playtest checkpoint:** longitudinal sessions with recorded routes and decisions. Do not claim the full
"very long time to godlike" promise is proven by a short accelerated demo.
