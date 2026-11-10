# Build diversity and replayability

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Shared interaction vocabulary

Start with a small number of well-defined states:

| State family | Core questions |
| --- | --- |
| Awareness | Who noticed what, where do they think it is, and how certain are they? |
| Position | What is in reach, what blocks a path, and where can each actor travel? |
| Commitment | Is an actor braced, exposed, preparing, recovering, or interruptible? |
| Protection | What does a ward or armor protect against, and what does maintaining it occupy? |
| Environment | Which local effects change movement, sight, or selected technique behavior? |

Add an interaction state only when multiple mechanics need a common authority for it. Avoid dozens of
one-off status names with almost identical behavior.

## Technique admission rule

Before adding a technique, answer:

- [ ] What useful thing does it do alone?
- [ ] Which two existing interactions make it more interesting?
- [ ] When is another available technique better?
- [ ] What information lets the player predict its result?
- [ ] What prevents repeated or stacked use from becoming the universal solution?
- [ ] Which identities can develop it, and why?
- [ ] Does it add a new decision rather than duplicate a stronger existing action?

Do not force every early prototype technique to ship with two completed partners before it can be tested.
The admission rule governs the intended small catalog as a whole.

## Build matrix

These are test identities, not classes the player selects:

| Identity | Main strengths | Main commitments | Characteristic rhythm |
| --- | --- | --- | --- |
| Close-range magic blaster | Concentration, burst channeling, protected entry | Short reach, exposure, recovery between major actions | Approach, survive commitment, release decisive force |
| Long-range shuriken specialist | Precision, trajectory control, distributed attention | Ammunition/recovery, space, interrupted control | Establish distance, place threats, exploit movement |
| Battlemage thief | Subtle expression, close coordination, selective protection | Peak output and sustain breadth | Mislead, enter quietly, strike, survive discovery |
| Warded duelist | Bracing, anticipation, close control | Limited pursuit and sustained commitments | Draw an attack, hold, answer an opening |
| Magical hunter | Awareness, ranged effects, route manipulation | Position dependence and limited close durability | Observe, separate, constrain, finish |
| Expedition healer/gatherer | Recovery, extraction, environmental knowledge | Direct destructive output | Avoid, endure, extract, preserve resources |
| Poison-stealth mage | Altered carriers, subtle expression, precise delivery | Delayed payoff, susceptibility, and revealed consequences | Observe, prepare, deliver quietly, reposition, exploit |
| Raging warded juggernaut | Prowess, Endurance, sustained protection | Turning, recovery, and finite protective capacity | Build momentum, commit, break through, manage the exposed interval |
| Area-of-effect frost mage | Cold evocation, anchors, distributed field control | Sustain load, setup, and concentration versus coverage | Establish geometry, constrain routes, concentrate at a decisive moment |
| Acrobatic projectile fighter | Movement, ranged delivery, timing | Landing space, trajectory, and load | Reposition, throw through an opening, redirect, recover |

Not every identity must defeat every apex at equal maturity. The matrix checks useful opportunities,
progression access, and coherent weaknesses, not identical performance.

## Hybrids need unique benefits

Split investment must not automatically create a worse specialist. Integrations should unlock benefits
that arise from the combination:

- Protection that lets a close caster safely complete a risky action.
- Quiet projection that reaches a target without giving every nearby creature an accurate source location.
- A physical throw that establishes an anchor for a later magical effect.
- Movement that preserves an ongoing effect instead of canceling it.

These benefits still have costs and evidence requirements. They are not automatic multipliers for collecting
more skill categories.

## Reaction and stacking rules

Before combinations expand:

- Every trigger has an owner, an event type, and a permitted timing phase.
- A reaction cannot recursively trigger itself on the same causal event.
- The core resolves an explicit bounded number of reactions in a stable order.
- Resource expenditure occurs before the dependent effect is committed.
- Sustain and attention limits apply throughout the sequence, not only at activation.
- Replacing an effect and refreshing an effect have separate rules.
- No combination creates Health, Focus, ammunition, or learning without an authorized source.
- Logs and previews expose why a reaction did or did not occur.

These are gameplay rules as well as implementation safeguards. A combo that only works because of ordering
ambiguity is not a successful synergy.

## New-character value

A second character should expose different decisions early, not just a different first weapon followed by
the same universal build.

Test identities through their actual development:

1. Start from broadly comparable potential.
2. Take different actions and discover different opportunities.
3. Commit different adaptations.
4. Return to the same controlled encounter.
5. Observe different viable strategies and different limits.

Keep this test separate from equipment-only loadout comparisons.

## Worked identity: poison-stealth mage

**Practiced threads:** Alteration, Veiling, Stealthcraft, and Precision. Ranged arms or close delivery can
become a later adjacent branch, but the build does not need to deeply master both.

**Beginning:** learn to approach and apply a modest supported carrier. Learn that a quiet approach, a valid
delivery, and a susceptible target are three different requirements.

**Development:** practice suppressing delivery signals and using delayed consequences to create openings.
Commit toward subtle output, exact carriers, and observation rather than large immediate blasts.

**Maturity:** prepare a sequence that separates discovery of the effect from discovery of its source.
Choose between delayed area pressure, a precise debuff, or a close decisive application according to what
the character actually developed.

**Refinement:** coordinate several supported marks or deliveries with deliberate timing. Lesser susceptible
foes can fall before they locate the character; an apex may demand probing defenses and planning an escape.

**Limits that preserve identity:** unsuitable physiology, a carrier blocked by protection, poor preparation
time, and sensitive observers. The answer is not to make all apex creatures poison-immune or force this
character to become a generic damage caster.

**Replayability hook:** an artifact suited to overt force or a visibly devastating juggernaut technique
suggests a different future without invalidating subtle mastery.

## Worked identity: raging juggernaut with protection spells

**Practiced threads:** Prowess, Endurance, and Warding. This is the clearest early expression of the Gath of
Baal reference: physical presence and supernatural protection reinforce one unmistakable identity.

**Beginning:** learn heavy commitment, bracing, and the value of protecting a vulnerable instant.

**Development:** keep a small ward coherent through forceful motion. Earn integration from real defended
commitments, not from putting on a buff before hitting an inert object.

**Maturity:** sustain protection while breaking a line, controlling space through mass and threat, and
choosing when to spend accumulated effort on a decisive attack.

**Refinement:** demolish lesser opposition, endure extraordinary impacts, and carry protection through
successive committed actions. Late choices concern how much power goes into breakthrough, sustained
survival, or protection of a wider area.

**Limits that preserve identity:** poor positioning can still be exploited; maximum protection is not
maximum mobility; sustaining a ward occupies capacity. The raging character retains input control and is
not arbitrarily forbidden from using magic.

**Replayability hook:** a narrow silent route or an agile enemy's movement suggests the acrobat or subtle
caster this character did not become.

## Worked identity: area-of-effect frost mage

**Practiced threads:** cold-focused Evocation and Binding, with a possible Warding branch. This is a weave
between magical methods, not a requirement to combine melee with magic.

**Beginning:** learn a small cold effect and how susceptible creatures respond.

**Development:** control placement and duration. Establish a supported anchor and observe how creatures
choose routes around a field.

**Maturity:** maintain a bounded area while directing concentrated effects at selected positions. Prepare
safe geometry instead of repeating the same projectile at every target.

**Refinement:** dominate ordinary groups across a large area, shape multiple supported zones, and choose
where to concentrate enough power to threaten an apex.

**Limits that preserve identity:** interrupted anchors, sustain load, cover, capable fliers or movers where
implemented, and resistant organisms. Counterplay comes from actual capabilities, not a universal boss
status-immunity flag.

**Replayability hook:** seeing what a close-range blaster can do with concentrated force creates interest
in a different specialization even within Evocation.

## Same tracks, different characters

Track membership is not the build. Two Evocation/Warding characters might become:

- A mobile close-range blaster whose ward protects brief committed entries.
- A slow siege caster whose anchored protection supports long preparation.
- A field controller whose wards delimit safe spaces around maintained effects.

Two physical characters practicing Prowess and Ranged arms might become:

- A heavy thrown-spear specialist using commitment and impact.
- A distributed projectile fighter whose precise coordination matters more than one heavy throw.

Their practice history, adaptations, integrations, equipment, and refinements decide the difference.
Do not let a diagram with a few track names become a disguised class selection screen.

## The replayability requirement: "I want to try this next"

Replayability is not the number of mathematically legal combinations. It is the number of materially
different, attractive lives the player can imagine living.

Support that through:

- Visible possibilities in found knowledge, relics, opponents, and related but unchosen developments.
- Early access to a small, useful version of a new character's intended behavior.
- Different decisions during travel and preparation, not only different combat animations.
- Strong refinement for the current character, so a new idea does not make the present one feel obsolete.
- Separate character saves, so experimentation does not destroy an old investment.
- Procedural sites and ecology that change the circumstances in which an identity operates.

A notebook may retain the player's own ideas locally without granting mechanical account-wide unlocks.
Automatic cross-character knowledge, inherited wealth, and shared-world progression remain deferred.

**Acceptance questions**

- [ ] Can a player name a different combination they want to try and describe how it would play?
- [ ] Did that desire arise from an interesting possibility rather than frustration with a broken build?
- [ ] Does the new character express a different behavior before the long mastery phase?
- [ ] Do the two characters still differ when given equally suitable equipment and comparable development?
- [ ] Does advancing either one continue to produce worthwhile refinement?
- [ ] Is each identity attractive for its own strengths, not merely because it counters the last boss?
- [ ] Has an uncoached player found a useful interaction the fixture designers did not suggest?
- [ ] Can that interaction earn development and survive save/load without a bespoke recipe being added?

---

## A practical breadth budget

The catalogs in the [discipline and technique rules](disciplines.md) describe the destination. Do not implement eleven complete progression trees at
once.

Use three expansion waves:

1. **Interaction kernel:** the existing CP2 identities prove close magic, physical range, subtle action, and
   protection with a few techniques.
2. **Weave proof:** a small subset of Alteration/Veiling, Prowess/Warding, and Evocation/Binding tests the three
   requested identities. Use controlled fixtures before adding entire branches.
3. **Development depth:** expand only the practices, commitments, and refinements needed to make those
   identities grow naturally through actual play.

The second wave is a checkpoint inside CP7, not a prerequisite for finishing CP1. It must include physical/
physical and magical/magical synergy as well as physical/magical synergy.
