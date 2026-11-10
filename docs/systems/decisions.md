# Decisions and assumptions

[Design index](../SYSTEMS_ROADMAP.md) · [Decisions](decisions.md) · [Checkpoints](checkpoints.md)

Proposed design. See the decisions document for committed direction versus prototype rules.

There are three kinds of statement in this document:

- **Committed direction:** settled by the design discussion. Implementations must preserve it.
- **Prototype rule:** a concrete starting design selected so it can be built and tested. It may change if
  evidence shows it fails the committed direction.
- **Tuning question:** a value or detailed balance decision that needs measurement and playtesting.

Do not treat an example technique name, a slot count, or a suggested resource model as user-approved balance.
Do not treat the committed direction as optional because a prototype rule is difficult to implement.

## Committed direction

- [x] The game is a classless, turn-based wanderer in a procedural living world.
- [x] Acting in a particular way gradually makes the character better suited to that way of acting.
- [x] Skills must support synergy and genuinely different playstyles.
- [x] Magic belongs throughout fighting, protection, gathering, movement, stealth, and healing.
- [x] Physical/magical, close/ranged, and overt/subtle are not predefined class bundles.
- [x] Close-range magical blasters and long-range shuriken specialists are valid identities.
- [x] Hybrids such as a battlemage thief must have their own strengths.
- [x] Multiple physical disciplines and multiple magical disciplines must have distinct practices and rules.
- [x] Weaving disciplines together is a major source of power, not merely stacking unrelated bonuses.
- [x] Barbarian-like force, acrobatics, stealth, ranged control, and close precision are different physical possibilities.
- [x] Poison-stealth magic, a raging juggernaut with protection spells, and area-of-effect frost magic are supported design targets.
- [x] Early potential is broad; later development becomes committed.
- [x] Radically changing a mature character's direction is not easy.
- [x] Starting a new character is a meaningful way to explore a different developmental direction.
- [x] Advancement comes from adventuring and practice, never elapsed real-world time.
- [x] Long-term development can produce godlike power.
- [x] Endgame growth refines a particular power rather than granting universal mastery.
- [x] Foes can also possess extraordinary power.
- [x] Ordinary foes remain ordinary enough to demonstrate the player's advancement.
- [x] Major encounters build toward unusually formidable apex foes.
- [x] OpenRouter can draft encounter arcs; the game owns their facts and mechanics.
- [x] Replayability centers on wanting to try a different combination on the next character.
- [x] Players should create viable combinations and builds the designers did not anticipate.
- [x] Named example builds are test cases, not an exhaustive menu or a whitelist of legal combinations.
- [x] Jev can be used extensively to interrogate the design; its judgments are not substitutes for actual play.

The checked items above record design decisions only. They do not indicate implemented code.

## Selected prototype rules

1. Character development has distinct foundations, adaptations, proficiencies, techniques, and integration.
2. Major adaptations consume finite structural capacity and require explicit commitment.
3. There is no ordinary respec that removes those commitments.
4. Techniques use a small, authored composition grammar.
5. Learning credit comes from resolved, meaningful activity, not raw button presses.
6. Health and Focus are the first resource model; Focus covers magical and physical special exertion.
7. Awareness, position, commitment, protection, and selected environmental states connect skills.
8. Home and camps support travel without forcing every discovery to end an expedition.
9. First encounter arcs are hand-authored fixtures using actual core mechanics.
10. Remote drafts enter an unrevealed site's candidate pipeline, never a running combat loop.
11. A revealed arc's mechanics are immutable; its runtime state can advance but cannot be rewritten by a
    late model response.
12. The first persistent character owns one world save. Separate new characters start separate worlds.
    Shared worlds and account-wide inheritance are deferred.
13. The initial discipline map uses five magical methods and six physical practices. Their names and count
    are provisional; only a small interaction slice ships first.
14. Disciplines organize practice and rules. They do not introduce extra class levels or generic point pools.
15. Composition legality comes from typed component contracts and shared interaction laws, not exact
    recipe membership. Player-facing structured composition is in scope; arbitrary code is not.

## Superseded ideas

The following earlier suggestions are explicitly not the current direction:

| Earlier suggestion | Replacement |
| --- | --- |
| Eventually learn and master everything on one character | Broad knowledge is possible, but finite developmental commitments prevent every mature identity on one character |
| A prepared skill bar is the primary specialization limit | Preparation limits immediate options; foundations also constrain what can be developed deeply |
| Mainly horizontal advancement forever | Modest early numerical growth, then genuine increases in specialized power, scale, and control |
| A conventional level-up tree defines character growth | Practice and developmental commitments define growth; a displayed overall rank, if added, is only a summary |
| More evolutionary history automatically means more danger | Danger comes from realized capabilities, resources, and encounter conditions, independently of player level |
| A bestiary is the main next gameplay reward | Knowledge supports combat, stealth, access, treasure, and development |
| Every generated species implies a generated boss | Apex encounters are exceptional, supported by world facts and deliberate encounter structure |
