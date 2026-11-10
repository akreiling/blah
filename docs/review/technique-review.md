# AI naming and Jev technique review

[Review process](README.md) · [Executable composition proof](../systems/composition-poc.md)

**Status:** integration contract, not a connected service. No naming model or Jev calls have been made for
the proof, and no model verdict has been obtained.

Keep three independent records:

| Record | Authority |
| --- | --- |
| Executable technique structure | Core component catalog, legality checks, costs, resolution, and tests |
| Display name and short description | Player or optional naming assistant; cosmetic only |
| Jev critique | Advisory evidence about coherence, exploits, costs, and missing tests |

“Backstab Burst Spine” and “Cinder Reckoning” are examples, not recipe IDs. A model-assisted rename must
not change power, eligibility, target anatomy, or execution. The current name field allows at most 48
characters and rejects empty names and control characters. The prototype UI does not call a naming model.

## Naming packet

Send only the supported structure and its actual behavior:

- Instrument, delivery, aim, payload, release, shape, and investment.
- What the core says it does, including limitations and costs.
- A requested naming style and a short output length.

Ask for a name and optional flavor description, not new effects. A description that promises homing,
invisibility, or immunity when those are not in the structure must not enter the mechanical record.
Keep the current name if the request fails. Naming never pauses combat.

## Jev review packet

Before a review, use the [budget and provider-verification procedure](README.md#review-procedure).
Include:

1. The code/rules revision and exact technique structure, independent of its name.
2. The relevant general interaction laws, not just the example's expected result.
3. Core legality results and costs, including failed attempts, empty areas, self-hits, and interruption rules.
4. Target facts and preparation conditions for each demonstrated case.
5. Reproducible commands, test assertions, actual outputs, and explicitly missing evidence.
6. A focused question, such as whether an interaction creates cost-free universal dominance.

The local `composition_demo` example produces concrete structures and outcomes suitable for an evidence
packet. Reviewers must also inspect the rules and adversarial tests rather than treating two happy-path
trials as comprehensive validation.

Ask Jev to return:

- The specific rule or assumption being challenged.
- Evidence or a concrete counterexample.
- Whether the concern is a contradiction, an exploit, an unclear explanation, or missing evidence.
- A discriminating test or playtest, rather than an unsupported balance score.

For the staff example, useful questions include whether Health plus Focus costs produce an interesting
choice, whether safe-home recovery or death makes the cost too easy to evade, and how lack of obstruction
changes its power. None has been answered by a model or a human playtest yet.

## What a verdict does

Jev can flag a lawful technique for review; it cannot silently outlaw a player's discovery. It can identify
a defect in the general rules; it cannot patch the live engine by returning prose.

Validate the criticism against actual code and a repro. If supported, fix the general law or cost and
version the rule change when persistence exists. Preserve surprising, costly, specialized power rather
than automatically reducing everything to expected damage.

Mechanical legality and advisory review remain separate statuses. A structurally invalid technique stays
invalid even if Jev likes it. A valid technique does not require a model's approval to execute. Service
failure leaves local composition, previews, and combat fully available.
