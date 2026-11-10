# CP8–CP11: living world and generated arcs

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## CP8. Make life a persistent history

**Goal:** replace independent region regrowth with durable ecological development.

**Work**

- [ ] Persist revealed populations and lineage records.
- [ ] Implement neighbor seeding from deterministic snapshots.
- [ ] Add game-time catch-up with explicit boundaries and bounded active work.
- [ ] Add historical observations and drift news.
- [ ] Connect exceptional individuals to world resources and recorded development.
- [ ] Make cache eviction reload saved facts.
- [ ] Protect revealed encounters from incompatible ecological rewrites.

**Exit evidence**

- [ ] Same recorded history replays identically.
- [ ] Cache capacity and lookup order do not change results.
- [ ] A death time jump does not grant player mastery or promote all foes indiscriminately.
- [ ] Ordinary and exceptional populations coexist under the rules.
- [ ] Catch-up has measured worst-case costs and tested fidelity.
- [ ] Old knowledge can become historical without the UI presenting it as a new certainty.
- [ ] Encounter participants have clear ownership when ecology and site state interact.

## CP9. Deepen creature capability diversity

**Goal:** biological diversity creates new gameplay rather than only more silhouettes or stats.

**Work**

- [ ] Identify missing encounter decisions that require additional biological traits.
- [ ] Add only the genes, organs, or behavior traits needed for those decisions.
- [ ] Define their fitness pressure and ecological cost.
- [ ] Derive capability eligibility from phenotype.
- [ ] Add readable visual and behavioral evidence.
- [ ] Test ordinary and exceptional expressions of the same capabilities.

**Exit evidence**

- [ ] Each addition changes at least one meaningful player decision.
- [ ] Every visible capability has actual rules and provenance.
- [ ] Related creatures remain recognizably related.
- [ ] Greater age or distance is not treated as automatic superiority.
- [ ] The same build faces different but understandable regional problems.
- [ ] The bestiary supports prediction without making every individual perfectly known.

## CP10. Add OpenRouter drafting behind the encounter contract

**Dependency:** CP7 and the persistent authored encounter pipeline. CP8/CP9 are not required if the snapshot
contains only the already implemented independent-life capabilities.

**Goal:** improve arc variety without outsourcing mechanical authority or blocking play.

**Work**

- [ ] Define and version the bounded input snapshot and output schema.
- [ ] Implement structural, world, and mechanical validation.
- [ ] Add fixture clients and deterministic offline fallback arcs.
- [ ] Add an opt-in generation adapter and durable spending/request ledger.
- [ ] Verify current provider APIs and selected model capabilities.
- [ ] Run an explicitly authorized small development batch.
- [ ] Inspect and play representative accepted outputs.
- [ ] Add bounded ahead-of-reveal scheduling only after that evidence is satisfactory.

**Exit evidence**

- [ ] Every accepted reference resolves to the provided catalog and world facts.
- [ ] Generated arcs contain no unsupported mechanics.
- [ ] No player build data is sent for counter-generation.
- [ ] Invalid, stale, late, duplicated, and missing responses are handled.
- [ ] Entering a site with a pending request commits a working fallback without waiting.
- [ ] Reloading never redrafts a revealed encounter.
- [ ] Quotas survive restart and ambiguous submission does not trigger automatic repeated spending.
- [ ] Tests and the default development build make no network calls.
- [ ] Accepted prose is coherent and the arcs have been played, not merely scored by a model.

**Stop or simplify if:** most drafts need mechanical invention, validation becomes permissive to accept
output, or generation costs grow faster than meaningful encounter variety.

## CP11. Validate the larger game before expanding again

**Goal:** establish that the integrated systems support the intended experience over time.

**Work**

- [ ] Run a documented multi-seed, multi-identity campaign test.
- [ ] Measure real startup, frontier turns, large encounters, save/load, and long-time catch-up.
- [ ] Check build diversity, progression pace, recovery pressure, and reward relevance.
- [ ] Evaluate arc repetition and offline quality.
- [ ] Verify core replay fixtures on every supported target/toolchain.
- [ ] Decide whether companions, breeding, towns, deeper crafting, or more remote art are now justified.

**Exit evidence**

- [ ] The main loop is compelling without remote services.
- [ ] Players can describe how their characters grew into different identities.
- [ ] Replayability comes from attractive new combinations, not forced restarts, hidden traps, or universal builds.
- [ ] At least one useful player-discovered combination was not envisioned in the authored fixture set.
- [ ] Powerful characters feel powerful outside exceptional encounters.
- [ ] Apex confrontations reward understanding and refinement.
- [ ] Long-term progress is meaningful without becoming universal mastery.
- [ ] The measured active working set and latency fit the declared budgets with headroom.
- [ ] Known limitations and failed gates are recorded rather than hidden by aggregate results.
