# Ecology and independent enemy power

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Two layers of creature development

**Lineage potential:** inherited anatomy, senses, movement, tolerances, and supported capabilities.

**Individual realization:** how much an individual has developed those capabilities through its history,
resources, age, and exceptional circumstances.

These layers let a lineage contain ordinary animals and exceptional specimens without making every member
a boss or every older creature infinitely stronger.

The first prototype can create exceptional specimens through deterministic, resource-supported world rules.
It need not simulate every offscreen battle of their lives before they are usable.

## From genes to capabilities

The existing eight genes already support visible differences. First make those differences affect play.

Only then add biological dimensions that support a missing decision, for example:

- A sensing distinction that matters to stealth.
- An anatomical structure that supports a readable attack.
- A protective or magical organ with an explicit cost.
- Social behavior that changes how a group acts.

Capability prerequisites should come from phenotype and world support, not a random boss ability list
unrelated to the creature.

Every new gene must justify its fitness pressure, phenotype effect, player-facing cue, and gameplay use.
Do not expand to 32 genes simply because the original design mentioned that scale.

## Extraordinary power needs support

An exceptional creature may require:

- A productive territory or unusual resource.
- A long development period.
- Rare inherited capacity.
- A specialized habitat.
- A magical site or persistent environmental relationship.

Power has upkeep, bottlenecks, and limits. Elapsed time alone cannot cause universal escalation.

This matters especially when death advances the world. Repeated deaths must not deterministically promote
every surrounding population beyond reach.

## No player-level scaling

Creature power is determined from world facts and its own recorded history. It is not computed from:

- The player's level or development rank.
- Equipped armor or damage.
- Which techniques are prepared.
- Whether the player has recently won or lost.

World generation can provide a plausible starting area and signal difficult sites. It cannot continually
rewrite established foes to keep every fight close.

## Distance, age, and danger

Distance creates exploration depth and unfamiliarity. Age creates ecological history. Neither is a direct
damage multiplier.

An old region can contain settled ordinary wildlife. A young or nearby site can contain an exceptional
danger. The important information is capability, context, and evidence.

Do not assume evolution is a ladder toward better combatants. Adaptation may produce specialization that
is strong in one habitat and weak in another.

## Persistent ecology

When neighbor seeding and drift arrive:

- First reveal records regional life as a world fact.
- Revisited regions catch up from game time through deterministic rules.
- The simulation reads a defined neighbor snapshot, not whichever neighbor a hash map happens to visit first.
- Learned species observations remain historical observations, with location and time.
- Speciation and important individual changes can produce news.
- Ordinary population updates and durable apex records have distinct ownership.
- Regeneration of evicted state reads saved facts, not the original independent-growth shortcut.

The sim must not claim that a displayed creature has capabilities its actual phenotype does not support.
