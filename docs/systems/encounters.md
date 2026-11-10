# Encounter arcs and apex creatures

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

## Encounters have shape

The intended arc:

1. **Attraction:** a landmark, rumor-like environmental cue, resource, or unusual sign invites investigation.
2. **Dominance:** lesser inhabitants demonstrate what the player can handle.
3. **Warning:** the site shows evidence of exceptional power.
4. **Escalation:** intermediate situations teach pieces of the apex's behavior.
5. **Preparation opportunity:** the player can observe, change approach, retreat, or investigate.
6. **Apex confrontation:** familiar rules combine at an extraordinary scale.
7. **Aftermath:** reward and a durable change close the arc.

An arc can omit a beat when appropriate. It cannot omit all warning and still call an unavoidable lethal
surprise fair.

## Peons have a purpose

Lesser foes should sometimes be easy. They provide contrast, texture, and evidence of growth.

They should not:

- Consume a long sequence of repetitive inputs.
- Become mandatory consumable taxes before every boss.
- Award enough repeatable mastery to replace difficult play.
- Inflate into synchronized player-scaled threats.

Later area effects and multi-target techniques can make superiority enjoyable. Their visual and input costs
must remain manageable in a tile-based UI.

## Apex design

A compelling apex has:

- A coherent source of power.
- A small set of readable capabilities.
- A distinctive relationship to its site.
- Threats that matter to multiple identities differently.
- Windows, constraints, or resources the player can exploit.
- An escape or avoidance story appropriate to the site's commitment point.
- A reward and aftermath that justify the buildup.

An apex can be overwhelming without being mechanically arbitrary.

## Difficulty is multidimensional

An encounter budget includes more than Health and damage:

- Action frequency and simultaneous threats.
- Range and coverage.
- Protection and interruption resistance.
- Mobility and control of routes.
- Detection and concealment.
- Environmental support.
- Summoning or group coordination.
- Telegraph quality and available recovery windows.

The engine computes these from legal instantiated mechanics. The drafter cannot claim an encounter is
within budget by writing a lower difficulty label.

## Build fairness

Fairness does not mean every character wins every encounter.

It means:

- A major threat has evidence before irreversible commitment.
- Useful interactions follow known rules.
- The intended family of viable approaches is not a single hidden key.
- A build can learn why it is unprepared and identify a plausible improvement or alternate goal.
- The generator does not inspect the current build and choose its counter.
- There are worthwhile destinations for different identities.

A stealth-oriented character may avoid an apex and take an accessible objective. Some supreme rewards may
still require resolving the central threat. State those reward conditions explicitly.

## Durable encounter state

Site state is not the same as currently loaded monsters:

```text
unrevealed
  -> revealed
  -> investigated
  -> engaged
  -> resolved
```

Retreat and abandonment are recorded transitions or timestamps within that lifecycle, not deletion of the
site. A resolved site's reward claims and lasting changes remain recorded.

Runtime state includes:

- Which clues and objectives have been discovered.
- Which objects or mechanisms have changed.
- Which persistent actors exist and their recovery state.
- Which rewards have been claimed.
- The current phase and pending committed actions.
- The last relevant simulation time.

Cache eviction, death, and application restart cannot substitute a newly rolled encounter.
